import { figure } from "../../src/dsl";
import { buildGhost } from "../ghost-shared";

// Stable depth-writing screen-door translucency for ordinary solid surfaces.
export default figure("ghost_dither", (api) => buildGhost(api, "dither"));
