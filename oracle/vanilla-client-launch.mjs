#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");
const REFERENCE_DIR = path.join(ROOT_DIR, "reference", "minecraft-1.17.1");
const ORACLE_DIR = path.join(ROOT_DIR, "oracle");
const CLASS_DIR = path.join(ORACLE_DIR, "classes");
const CLIENT_DEOBF_JAR = path.join(REFERENCE_DIR, "client-deobf.jar");
const VERSION_JSON_PATH = path.join(REFERENCE_DIR, "1.17.1.json");
const LIBRARY_DIR = path.join(REFERENCE_DIR, "libraries");
const DEFAULT_ASSETS_DIR = path.join(REFERENCE_DIR, "assets");
const DEFAULT_NATIVES_DIR = path.join(REFERENCE_DIR, "natives", minecraftOsName());
const DEFAULT_GAME_DIR = path.join(os.tmpdir(), "mclone-vanilla-client");

main().catch((error) => {
  console.error(`error: ${error.message}`);
  process.exit(1);
});

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const version = readVersionJson();
  const assetIndex = version.assetIndex;
  if (!assetIndex?.id || !assetIndex?.url) {
    throw new Error(`missing asset index in ${VERSION_JSON_PATH}`);
  }

  if (!options.noBuild) {
    runBuild();
  }

  const libraries = allowedLibraries(version);
  const classpath = buildClasspath(libraries);

  if (options.hydrate || options.launch) {
    await hydrateNatives(libraries, options);
    await hydrateAssets(assetIndex, options);
  }

  const javaCommand = buildJavaCommand({
    classpath,
    nativesDir: options.nativesDir,
    gameDir: options.gameDir,
    assetsDir: options.assetsDir,
    assetIndex: assetIndex.id,
    width: options.width,
    height: options.height,
    username: options.username,
    uuid: options.uuid,
    accessToken: options.accessToken,
    launch: options.launch,
    printArgs: options.printArgs,
  });

  if (options.printCommand) {
    console.log(javaCommand.map(shellQuote).join(" "));
  }

  if (options.printArgs || options.launch) {
    fs.mkdirSync(options.gameDir, { recursive: true });
    const result = spawnSync(javaCommand[0], javaCommand.slice(1), {
      cwd: ROOT_DIR,
      stdio: "inherit",
    });
    if (result.status !== 0) {
      throw new Error(`java exited with status ${result.status}`);
    }
  }
}

function parseArgs(args) {
  const options = {
    accessToken: "0",
    assetsDir: DEFAULT_ASSETS_DIR,
    gameDir: DEFAULT_GAME_DIR,
    height: "480",
    hydrate: false,
    launch: false,
    nativesDir: DEFAULT_NATIVES_DIR,
    noBuild: false,
    noDownload: false,
    printArgs: false,
    printCommand: false,
    username: "McloneOracle",
    uuid: null,
    width: "854",
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    switch (arg) {
      case "--":
        break;
      case "--help":
      case "-h":
        printUsage();
        process.exit(0);
        break;
      case "--hydrate":
        options.hydrate = true;
        break;
      case "--launch":
        options.launch = true;
        break;
      case "--print-args":
        options.printArgs = true;
        break;
      case "--print-command":
        options.printCommand = true;
        break;
      case "--no-build":
        options.noBuild = true;
        break;
      case "--no-download":
        options.noDownload = true;
        break;
      case "--game-dir":
        options.gameDir = requireValue(args, ++index, arg);
        break;
      case "--assets-dir":
        options.assetsDir = requireValue(args, ++index, arg);
        break;
      case "--natives-dir":
        options.nativesDir = requireValue(args, ++index, arg);
        break;
      case "--username":
        options.username = requireValue(args, ++index, arg);
        break;
      case "--uuid":
        options.uuid = requireValue(args, ++index, arg);
        break;
      case "--access-token":
        options.accessToken = requireValue(args, ++index, arg);
        break;
      case "--width":
        options.width = requirePositiveInteger(requireValue(args, ++index, arg), "width");
        break;
      case "--height":
        options.height = requirePositiveInteger(requireValue(args, ++index, arg), "height");
        break;
      default:
        throw new Error(`unsupported option '${arg}'`);
    }
  }

  if (!options.printCommand && !options.printArgs && !options.launch && !options.hydrate) {
    options.printCommand = true;
  }

  if (options.launch && options.noDownload) {
    assertRuntimePrepared(options);
  }

  return options;
}

function printUsage() {
  console.log(`usage: node oracle/vanilla-client-launch.mjs [options]

Default action prints the Java command without launching Minecraft.

options:
  --print-command        print the java command line
  --print-args           run the wrapper only far enough to print vanilla Main args
  --hydrate              download assets and macOS natives, then exit
  --launch               launch the deobfuscated vanilla client explicitly
  --no-build             skip oracle/build.sh
  --no-download          do not download missing assets or natives
  --game-dir <path>      default: ${DEFAULT_GAME_DIR}
  --assets-dir <path>    default: ${DEFAULT_ASSETS_DIR}
  --natives-dir <path>   default: ${DEFAULT_NATIVES_DIR}
  --username <name>      default: McloneOracle
  --uuid <uuid>          default: offline UUID derived from username
  --access-token <tok>   default: 0
  --width <pixels>       default: 854
  --height <pixels>      default: 480`);
}

function readVersionJson() {
  if (!fs.existsSync(CLIENT_DEOBF_JAR)) {
    throw new Error(`missing ${CLIENT_DEOBF_JAR}; run scripts/decompile-mc.sh first`);
  }
  if (!fs.existsSync(VERSION_JSON_PATH)) {
    throw new Error(`missing ${VERSION_JSON_PATH}; run scripts/decompile-mc.sh first`);
  }
  return JSON.parse(fs.readFileSync(VERSION_JSON_PATH, "utf8"));
}

function runBuild() {
  const result = spawnSync(path.join(ORACLE_DIR, "build.sh"), {
    cwd: ROOT_DIR,
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw new Error(`oracle/build.sh exited with status ${result.status}`);
  }
}

function allowedLibraries(version) {
  return (version.libraries ?? []).filter((library) => allowByRules(library.rules));
}

function allowByRules(rules) {
  if (!rules || rules.length === 0) {
    return true;
  }

  let allowed = false;
  for (const rule of rules) {
    if (ruleMatches(rule)) {
      allowed = rule.action === "allow";
    }
  }
  return allowed;
}

function ruleMatches(rule) {
  if (rule.os) {
    if (rule.os.name && rule.os.name !== minecraftOsName()) {
      return false;
    }
    if (rule.os.arch && rule.os.arch !== minecraftArchName()) {
      return false;
    }
    if (rule.os.version) {
      const regex = new RegExp(rule.os.version);
      if (!regex.test(os.release())) {
        return false;
      }
    }
  }

  if (rule.features) {
    return false;
  }

  return true;
}

function buildClasspath(libraries) {
  const entries = [CLASS_DIR, CLIENT_DEOBF_JAR];
  const seen = new Set(entries);
  for (const library of libraries) {
    const artifact = library.downloads?.artifact;
    if (!artifact?.path) {
      continue;
    }
    const jarPath = path.join(LIBRARY_DIR, artifact.path);
    if (!fs.existsSync(jarPath)) {
      throw new Error(`missing library ${jarPath}; run oracle/build.sh`);
    }
    if (!seen.has(jarPath)) {
      entries.push(jarPath);
      seen.add(jarPath);
    }
  }
  return entries.join(path.delimiter);
}

async function hydrateNatives(libraries, options) {
  const nativeDownloads = [];
  const seen = new Set();
  for (const library of libraries) {
    const classifier = nativeClassifier(library);
    if (!classifier) {
      continue;
    }
    const download = library.downloads?.classifiers?.[classifier];
    if (download?.url && download?.path && !seen.has(download.path)) {
      nativeDownloads.push(download);
      seen.add(download.path);
    }
  }

  if (nativeDownloads.length === 0) {
    throw new Error(`no native classifiers found for ${minecraftOsName()}`);
  }

  fs.mkdirSync(options.nativesDir, { recursive: true });
  for (const download of nativeDownloads) {
    const jarPath = path.join(LIBRARY_DIR, download.path);
    await ensureDownload(download.url, jarPath, download.sha1, options);
    extractNativeJar(jarPath, options.nativesDir);
  }
}

async function hydrateAssets(assetIndex, options) {
  const indexPath = path.join(options.assetsDir, "indexes", `${assetIndex.id}.json`);
  await ensureDownload(assetIndex.url, indexPath, assetIndex.sha1, options);

  const index = JSON.parse(fs.readFileSync(indexPath, "utf8"));
  const objects = Object.values(index.objects ?? {});
  let downloaded = 0;
  for (const object of objects) {
    if (!object.hash) {
      continue;
    }
    const prefix = object.hash.slice(0, 2);
    const objectPath = path.join(options.assetsDir, "objects", prefix, object.hash);
    const objectUrl = `https://resources.download.minecraft.net/${prefix}/${object.hash}`;
    const wasMissing = !fs.existsSync(objectPath);
    await ensureDownload(objectUrl, objectPath, object.hash, options);
    if (wasMissing) {
      downloaded += 1;
      if (downloaded % 250 === 0) {
        console.error(`downloaded ${downloaded} assets...`);
      }
    }
  }
}

function nativeClassifier(library) {
  const natives = library.natives;
  if (!natives) {
    return null;
  }
  const template = natives[minecraftOsName()];
  if (!template) {
    return null;
  }
  return template.replace("${arch}", minecraftArchBits());
}

function buildJavaCommand({ classpath, nativesDir, gameDir, assetsDir, assetIndex, width, height, username, uuid, accessToken, launch, printArgs }) {
  const command = ["java"];
  if (minecraftOsName() === "osx") {
    command.push("-XstartOnFirstThread");
  }
  command.push(`-Djava.library.path=${nativesDir}`);
  command.push("-Dminecraft.launcher.brand=mclone-oracle");
  command.push("-Dminecraft.launcher.version=0");
  command.push("-cp", classpath, "VanillaClientLauncher");
  if (launch) {
    command.push("--mclone-launch");
  }
  if (printArgs) {
    command.push("--mclone-print-args");
  }
  command.push("--mclone-game-dir", gameDir);
  command.push("--mclone-assets-dir", assetsDir);
  command.push("--mclone-asset-index", assetIndex);
  command.push("--mclone-username", username);
  if (uuid) {
    command.push("--mclone-uuid", uuid);
  }
  command.push("--mclone-access-token", accessToken);
  command.push("--mclone-width", width);
  command.push("--mclone-height", height);
  command.push("--mclone-disable-multiplayer");
  command.push("--mclone-disable-chat");
  return command;
}

function extractNativeJar(jarPath, nativesDir) {
  const result = spawnSync("unzip", ["-q", "-o", jarPath, "-d", nativesDir], {
    cwd: ROOT_DIR,
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw new Error(`failed to extract ${jarPath}; install unzip or extract the native jar manually`);
  }
}

async function ensureDownload(url, destination, sha1, options) {
  if (fs.existsSync(destination)) {
    if (sha1) {
      const actual = sha1File(destination);
      if (actual !== sha1) {
        throw new Error(`sha1 mismatch for ${destination}: expected ${sha1}, got ${actual}`);
      }
    }
    return;
  }

  if (options.noDownload) {
    throw new Error(`missing ${destination} and --no-download was passed`);
  }

  fs.mkdirSync(path.dirname(destination), { recursive: true });
  const temporary = `${destination}.tmp-${process.pid}`;
  await download(url, temporary);
  if (sha1) {
    const actual = sha1File(temporary);
    if (actual !== sha1) {
      fs.rmSync(temporary, { force: true });
      throw new Error(`sha1 mismatch for ${url}: expected ${sha1}, got ${actual}`);
    }
  }
  fs.renameSync(temporary, destination);
}

function download(url, destination) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        download(response.headers.location, destination).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`download failed ${response.statusCode}: ${url}`));
        return;
      }
      const output = fs.createWriteStream(destination);
      response.pipe(output);
      output.on("finish", () => output.close(resolve));
      output.on("error", reject);
    });
    request.on("error", reject);
  });
}

function sha1File(filePath) {
  const hash = crypto.createHash("sha1");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function assertRuntimePrepared(options) {
  if (!fs.existsSync(options.assetsDir)) {
    throw new Error(`assets dir is missing: ${options.assetsDir}`);
  }
  if (!fs.existsSync(options.nativesDir)) {
    throw new Error(`natives dir is missing: ${options.nativesDir}`);
  }
}

function requireValue(args, index, option) {
  if (index >= args.length || args[index].startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return args[index];
}

function requirePositiveInteger(value, name) {
  if (!/^[1-9][0-9]*$/.test(value)) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function minecraftOsName() {
  switch (process.platform) {
    case "darwin":
      return "osx";
    case "win32":
      return "windows";
    case "linux":
      return "linux";
    default:
      return process.platform;
  }
}

function minecraftArchName() {
  switch (process.arch) {
    case "ia32":
      return "x86";
    case "x64":
      return "x86_64";
    case "arm64":
      return "arm64";
    default:
      return process.arch;
  }
}

function minecraftArchBits() {
  return process.arch === "ia32" ? "32" : "64";
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_/:=.,@%+-]+$/.test(value)) {
    return value;
  }
  return `'${value.replaceAll("'", "'\\''")}'`;
}
