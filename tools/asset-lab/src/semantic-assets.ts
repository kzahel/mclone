export const SEMANTIC_ASSET_USES = ["actor", "world_prop", "item_prop", "trace_prop"] as const;
export type SemanticAssetUse = typeof SEMANTIC_ASSET_USES[number];

export const SEMANTIC_ASSET_ANCHORS = ["feet", "ground", "item_center", "surface_trace"] as const;
export type SemanticAssetAnchor = typeof SEMANTIC_ASSET_ANCHORS[number];

export const SEMANTIC_INSTANTIATION_STATUSES = ["live_gameplay", "review_only"] as const;
export type SemanticInstantiationStatus = typeof SEMANTIC_INSTANTIATION_STATUSES[number];

export function semanticAssetAnchorForUse(use: SemanticAssetUse): SemanticAssetAnchor {
  switch (use) {
    case "actor": return "feet";
    case "world_prop": return "ground";
    case "item_prop": return "item_center";
    case "trace_prop": return "surface_trace";
  }
}
