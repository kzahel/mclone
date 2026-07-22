import { figure } from "../../src/dsl";
import { buildGhost } from "../ghost-shared";

// Smooth depth-prepass translucency with additive eyes and trailing aura.
export default figure("ghost_translucent", (api) => buildGhost(api, "blend"));
