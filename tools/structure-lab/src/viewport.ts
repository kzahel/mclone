import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import type { StructureCatalogEntry } from "./catalog-model";

export type CameraPreset = "three-quarter" | "front" | "side" | "top";
export type ViewportStatus = "loading" | "ready" | "error";

export interface StructureViewportOptions {
  background: "light" | "dark";
  onStatus?: (status: ViewportStatus, error?: string) => void;
}

export class StructureViewportController {
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.PerspectiveCamera(34, 1, 0.05, 1_000);
  private readonly renderer: THREE.WebGLRenderer;
  private readonly controls: OrbitControls;
  private readonly resizeObserver: ResizeObserver;
  private readonly options: StructureViewportOptions;
  private readonly grid = new THREE.GridHelper(48, 48, 0x7f947e, 0xb8c4b5);
  private pivot: THREE.Group | null = null;
  private content: THREE.Group | null = null;
  private boundsHelper: THREE.Box3Helper | null = null;
  private markerGroup: THREE.Group | null = null;
  private clippingPlane = new THREE.Plane(new THREE.Vector3(0, -1, 0), 512);
  private materials = new Set<THREE.Material>();
  private textures = new Set<THREE.Texture>();
  private componentNodes = new Map<string, THREE.Object3D>();
  private entry: StructureCatalogEntry | null = null;
  private cameraPreset: CameraPreset = "three-quarter";
  private loadGeneration = 0;
  private renderQueued = false;

  constructor(host: HTMLElement, options: StructureViewportOptions) {
    this.options = options;
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: "high-performance" });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.localClippingEnabled = true;
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.domElement.className = "structureViewportCanvas";
    this.renderer.domElement.dataset.cameraPreset = this.cameraPreset;
    this.renderer.domElement.dataset.viewerStatus = "loading";
    host.append(this.renderer.domElement);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = false;
    this.controls.minDistance = 2;
    this.controls.maxDistance = 160;
    this.controls.addEventListener("change", this.scheduleRender);
    this.grid.position.y = -0.02;
    this.scene.add(this.grid);
    this.setTheme(options.background);
    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(host);
    this.resize();
  }

  async load(entry: StructureCatalogEntry): Promise<void> {
    const generation = ++this.loadGeneration;
    this.setStatus("loading");
    try {
      const meshUrl = assetUrl(entry.meshPath);
      const response = await fetch(meshUrl, { cache: "no-cache" });
      if (!response.ok) throw new Error(`Could not load preview mesh: HTTP ${response.status}`);
      const bytes = await response.arrayBuffer();
      const actualHash = await sha256(bytes);
      if (actualHash !== entry.artifacts.meshSha256) {
        throw new Error(`Preview mesh '${entry.structureId}' failed its SHA-256 check`);
      }
      const gltf = await parseGltf(bytes, directoryUrl(meshUrl));
      if (generation !== this.loadGeneration) {
        disposeObject(gltf.scene);
        return;
      }
      this.clearStructure();
      this.entry = entry;
      this.pivot = new THREE.Group();
      this.pivot.name = `structure:${entry.structureId}`;
      this.content = new THREE.Group();
      const centerX = entry.size[0] * 0.5;
      const centerZ = entry.size[2] * 0.5;
      this.pivot.position.set(centerX, 0, centerZ);
      this.content.position.set(-centerX, 0, -centerZ);
      this.content.add(gltf.scene);
      this.pivot.add(this.content);
      this.scene.add(this.pivot);
      gltf.scene.traverse((object) => {
        if (object.name.startsWith("component:")) {
          this.componentNodes.set(object.name.slice("component:".length), object);
        }
        if (object instanceof THREE.Mesh) {
          const materials = Array.isArray(object.material) ? object.material : [object.material];
          for (const material of materials) this.prepareMaterial(material);
        }
      });
      this.boundsHelper = new THREE.Box3Helper(
        new THREE.Box3(
          new THREE.Vector3(0, 0, 0),
          new THREE.Vector3(entry.size[0], entry.size[1], entry.size[2]),
        ),
        0x315e45,
      );
      this.boundsHelper.visible = false;
      this.content.add(this.boundsHelper);
      this.markerGroup = buildMarkers(entry);
      this.markerGroup.visible = true;
      this.content.add(this.markerGroup);
      this.renderer.domElement.dataset.structure = entry.structureId;
      this.setLayer(entry.size[1] - 1);
      this.setTransform(0, false);
      this.setCameraPreset(this.cameraPreset);
      this.setStatus("ready");
    } catch (error) {
      if (generation !== this.loadGeneration) return;
      this.setStatus("error", error instanceof Error ? error.message : String(error));
      throw error;
    }
  }

  setTheme(theme: "light" | "dark"): void {
    const colors = theme === "dark"
      ? { center: 0x6f8b72, grid: 0x3e5342 }
      : { center: 0x839c82, grid: 0xc4cdbd };
    const material = this.grid.material as THREE.LineBasicMaterial;
    const colorsAttribute = this.grid.geometry.getAttribute("color");
    if (colorsAttribute instanceof THREE.BufferAttribute) {
      const center = new THREE.Color(colors.center);
      const grid = new THREE.Color(colors.grid);
      for (let index = 0; index < colorsAttribute.count; index += 1) {
        const useCenter = index < 8;
        const color = useCenter ? center : grid;
        colorsAttribute.setXYZ(index, color.r, color.g, color.b);
      }
      colorsAttribute.needsUpdate = true;
    }
    material.opacity = theme === "dark" ? 0.55 : 0.72;
    material.transparent = true;
    this.scheduleRender();
  }

  setCameraPreset(preset: CameraPreset): void {
    this.cameraPreset = preset;
    this.renderer.domElement.dataset.cameraPreset = preset;
    this.fit(preset);
  }

  fit(preset: CameraPreset = this.cameraPreset): void {
    const entry = this.entry;
    if (!entry) return;
    const target = new THREE.Vector3(0, entry.size[1] * 0.42, 0);
    const radius = Math.hypot(...entry.size) * 0.52;
    const direction = cameraDirection(preset).normalize();
    const distance = Math.max(8, radius / Math.tan(THREE.MathUtils.degToRad(this.camera.fov * 0.48)));
    this.camera.position.copy(target).addScaledVector(direction, distance);
    this.camera.near = Math.max(0.05, distance - radius * 2.2);
    this.camera.far = distance + radius * 4;
    this.camera.updateProjectionMatrix();
    this.controls.target.copy(target);
    this.controls.update();
    this.scheduleRender();
  }

  setLayer(maxY: number): void {
    this.clippingPlane.constant = Math.max(0, maxY + 1.001);
    this.renderer.domElement.dataset.layer = String(maxY);
    this.scheduleRender();
  }

  setComponentVisible(id: string, visible: boolean): void {
    const node = this.componentNodes.get(id);
    if (node) node.visible = visible;
    this.scheduleRender();
  }

  setMarkersVisible(visible: boolean): void {
    if (this.markerGroup) this.markerGroup.visible = visible;
    this.scheduleRender();
  }

  setBoundsVisible(visible: boolean): void {
    if (this.boundsHelper) this.boundsHelper.visible = visible;
    this.scheduleRender();
  }

  setTransform(rotation: 0 | 90 | 180 | 270, mirror: boolean): void {
    if (!this.pivot) return;
    this.pivot.rotation.y = THREE.MathUtils.degToRad(rotation);
    this.pivot.scale.x = mirror ? -1 : 1;
    this.renderer.domElement.dataset.rotation = String(rotation);
    this.renderer.domElement.dataset.mirror = String(mirror);
    this.scheduleRender();
  }

  renderNow(): void {
    this.resize();
    this.renderer.render(this.scene, this.camera);
  }

  dispose(): void {
    this.loadGeneration += 1;
    this.resizeObserver.disconnect();
    this.controls.removeEventListener("change", this.scheduleRender);
    this.controls.dispose();
    this.clearStructure();
    this.grid.geometry.dispose();
    (this.grid.material as THREE.Material).dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }

  private prepareMaterial(material: THREE.Material): void {
    material.clippingPlanes = [this.clippingPlane];
    material.clipShadows = false;
    this.materials.add(material);
    if ("map" in material && material.map instanceof THREE.Texture) {
      material.map.magFilter = THREE.NearestFilter;
      material.map.minFilter = THREE.NearestFilter;
      material.map.generateMipmaps = false;
      material.map.needsUpdate = true;
      this.textures.add(material.map);
    }
  }

  private clearStructure(): void {
    if (this.pivot) {
      this.scene.remove(this.pivot);
      this.pivot.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.LineSegments) {
          object.geometry.dispose();
          const objectMaterials = Array.isArray(object.material)
            ? object.material
            : [object.material];
          for (const material of objectMaterials) {
            this.materials.add(material);
            if ("map" in material && material.map instanceof THREE.Texture) {
              this.textures.add(material.map);
            }
          }
        }
      });
    }
    for (const material of this.materials) material.dispose();
    for (const texture of this.textures) texture.dispose();
    this.materials.clear();
    this.textures.clear();
    this.componentNodes.clear();
    this.pivot = null;
    this.content = null;
    this.boundsHelper = null;
    this.markerGroup = null;
    this.entry = null;
  }

  private readonly scheduleRender = (): void => {
    if (this.renderQueued) return;
    this.renderQueued = true;
    requestAnimationFrame(() => {
      this.renderQueued = false;
      this.renderer.render(this.scene, this.camera);
    });
  };

  private resize(): void {
    const host = this.renderer.domElement.parentElement;
    if (!host) return;
    const width = Math.max(1, host.clientWidth);
    const height = Math.max(1, host.clientHeight);
    const size = new THREE.Vector2();
    this.renderer.getSize(size);
    if (size.x !== width || size.y !== height) {
      this.renderer.setSize(width, height, false);
      this.camera.aspect = width / height;
      this.camera.updateProjectionMatrix();
    }
    this.scheduleRender();
  }

  private setStatus(status: ViewportStatus, error?: string): void {
    this.renderer.domElement.dataset.viewerStatus = status;
    this.options.onStatus?.(status, error);
    this.scheduleRender();
  }
}

function buildMarkers(entry: StructureCatalogEntry): THREE.Group {
  const group = new THREE.Group();
  group.name = "structure-markers";
  const markerMaterial = new THREE.MeshBasicMaterial({ color: 0xdb5b3d });
  const socketMaterial = new THREE.MeshBasicMaterial({ color: 0x3c8a78 });
  for (const marker of entry.markers) {
    const mesh = new THREE.Mesh(new THREE.OctahedronGeometry(0.24, 0), markerMaterial.clone());
    mesh.position.set(marker.pos[0] + 0.5, marker.pos[1] + 0.7, marker.pos[2] + 0.5);
    mesh.name = `marker:${marker.kind}`;
    group.add(mesh);
  }
  for (const socket of entry.sockets) {
    const mesh = new THREE.Mesh(new THREE.TorusGeometry(0.28, 0.07, 6, 16), socketMaterial.clone());
    mesh.position.set(socket.pos[0] + 0.5, socket.pos[1] + 0.5, socket.pos[2] + 0.5);
    mesh.rotation.x = Math.PI / 2;
    mesh.name = `socket:${socket.id}`;
    group.add(mesh);
  }
  markerMaterial.dispose();
  socketMaterial.dispose();
  return group;
}

function cameraDirection(preset: CameraPreset): THREE.Vector3 {
  switch (preset) {
    case "front": return new THREE.Vector3(0, 0.22, 1);
    case "side": return new THREE.Vector3(1, 0.22, 0);
    case "top": return new THREE.Vector3(0.001, 1, 0.001);
    case "three-quarter": return new THREE.Vector3(1, 0.62, 1.18);
  }
}

function parseGltf(bytes: ArrayBuffer, resourcePath: string): Promise<{ scene: THREE.Group }> {
  return new Promise((resolve, reject) => {
    new GLTFLoader().parse(bytes, resourcePath, resolve, reject);
  });
}

function directoryUrl(url: string): string {
  return url.slice(0, url.lastIndexOf("/") + 1);
}

async function sha256(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function assetUrl(relativePath: string): string {
  return `${import.meta.env.BASE_URL}${relativePath}`;
}

function disposeObject(root: THREE.Object3D): void {
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  root.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      object.geometry.dispose();
      const objectMaterials = Array.isArray(object.material) ? object.material : [object.material];
      for (const material of objectMaterials) {
        materials.add(material);
        if ("map" in material && material.map instanceof THREE.Texture) textures.add(material.map);
      }
    }
  });
  for (const material of materials) material.dispose();
  for (const texture of textures) texture.dispose();
}
