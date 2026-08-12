import { figure } from "../../src/dsl";

// A compact carried bee-hotel kit. The strapped boards echo the full world
// prop without pretending the inventory entity is already assembled.
export default figure("bee_hotel_item", ({ box, mat, part }) => {
  mat("wood", "#a87540");
  mat("wood_light", "#d1a665");
  mat("strap", "#5c412b");
  mat("hole", "#2c2118");

  for (const [name, x] of [["left", -0.2], ["middle", 0], ["right", 0.2]] as const) {
    part(`board_${name}`, box({ at: [x, 0, 0], size: [0.16, 0.34, 0.72], material: "wood" }));
    part(`hole_${name}`, box({ at: [x, 0.05, -0.37], size: [0.065, 0.065, 0.025], material: "hole" }));
  }
  part("cap", box({ at: [0, 0.2, 0], size: [0.68, 0.08, 0.78], material: "wood_light" }));
  part("strap", box({ at: [0, 0.245, 0], size: [0.1, 0.055, 0.84], material: "strap" }));
});
