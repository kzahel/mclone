// Ratcheted initial warning inventory. Update only while reviewing surface-check output.
export const SURFACE_BASELINE: Readonly<Record<string, readonly string[]>> = {
  "american_bison": [
    "coplanar-overlap:leg_bl.west|rear.west",
    "coplanar-overlap:leg_br.east|rear.east"
  ],
  "bearfolk": [
    "coplanar-overlap:paw_l.north|torso.north",
    "coplanar-overlap:paw_r.north|torso.north"
  ],
  "beaver": [
    "coplanar-overlap:belly.north|leg_fl.north",
    "coplanar-overlap:belly.north|leg_fr.north"
  ],
  "camel": [
    "coplanar-overlap:chest.east|leg_fr.east",
    "coplanar-overlap:chest.west|leg_fl.west"
  ],
  "capuchin_monkey": [
    "coplanar-overlap:head.down|muzzle.down"
  ],
  "cat": [
    "coplanar-overlap:leg_bl.south|paw_bl.south",
    "coplanar-overlap:leg_br.south|paw_br.south",
    "coplanar-overlap:leg_fl.south|paw_fl.south",
    "coplanar-overlap:leg_fr.south|paw_fr.south"
  ],
  "cheetah": [
    "coplanar-overlap:body.north|deep_chest.north",
    "coplanar-overlap:body.south|raised_hips.south",
    "coplanar-overlap:deep_chest.east|leg_fr.east",
    "coplanar-overlap:deep_chest.west|leg_fl.west"
  ],
  "cow": [
    "coplanar-overlap:hoof_bl.south|leg_bl.south",
    "coplanar-overlap:hoof_br.south|leg_br.south",
    "coplanar-overlap:hoof_fl.south|leg_fl.south",
    "coplanar-overlap:hoof_fr.south|leg_fr.south"
  ],
  "crab": [
    "coplanar-overlap:carapace.down|rear_plate.down"
  ],
  "crocodile": [
    "coplanar-overlap:belly.south|leg_bl.south",
    "coplanar-overlap:belly.south|leg_br.south",
    "coplanar-overlap:eye_mound_l.west|head.west",
    "coplanar-overlap:eye_mound_r.east|head.east"
  ],
  "earwig": [
    "coplanar-overlap:abdomen_band_3.east|abdomen.east",
    "coplanar-overlap:abdomen_band_3.west|abdomen.west"
  ],
  "echidna": [
    "coplanar-overlap:body.down|head.down"
  ],
  "fox": [
    "coplanar-overlap:body.down|chest.down"
  ],
  "frog": [
    "coplanar-overlap:body.up|head.up"
  ],
  "gecko": [
    "coplanar-overlap:belly.north|leg_front_l.north",
    "coplanar-overlap:belly.north|leg_front_r.north"
  ],
  "giant_anteater": [
    "coplanar-overlap:belly.north|leg_fl.north",
    "coplanar-overlap:belly.north|leg_fr.north",
    "coplanar-overlap:leg_bl.south|paw_bl.south",
    "coplanar-overlap:leg_br.south|paw_br.south"
  ],
  "harbor_seal": [
    "coplanar-overlap:body.down|rump.down"
  ],
  "hedgehog": [
    "coplanar-overlap:body.down|head.down",
    "coplanar-overlap:leg_bl.south|paw_bl.south",
    "coplanar-overlap:leg_br.south|paw_br.south",
    "coplanar-overlap:leg_fl.south|paw_fl.south",
    "coplanar-overlap:leg_fr.south|paw_fr.south"
  ],
  "jaguar": [
    "coplanar-overlap:cheek_l.down|head.down",
    "coplanar-overlap:cheek_r.down|head.down"
  ],
  "jellyfish": [
    "coplanar-overlap:bell_lower.east|rim_right.east",
    "coplanar-overlap:bell_lower.north|rim_front.north",
    "coplanar-overlap:bell_lower.south|rim_back.south",
    "coplanar-overlap:bell_lower.west|rim_left.west"
  ],
  "king_cobra": [
    "coplanar-overlap:body_2.down|body_3.down",
    "coplanar-overlap:body_3.down|body_4.down",
    "coplanar-overlap:head.down|muzzle.down"
  ],
  "koala": [
    "coplanar-overlap:ear_l.up|head.up",
    "coplanar-overlap:ear_r.up|head.up",
    "coplanar-overlap:foot_l.south|leg_l.south",
    "coplanar-overlap:foot_r.south|leg_r.south"
  ],
  "komodo_dragon": [
    "coplanar-overlap:body.up|neck_1.up"
  ],
  "lionfolk": [
    "coplanar-overlap:arm_l.south|hand_l.south",
    "coplanar-overlap:arm_r.south|hand_r.south"
  ],
  "malayan_tapir": [
    "coplanar-overlap:body.down|rump.down"
  ],
  "mouse": [
    "coplanar-overlap:body.east|leg_fr.east",
    "coplanar-overlap:body.west|leg_fl.west",
    "coplanar-overlap:head.down|muzzle.down"
  ],
  "orangutan": [
    "coplanar-overlap:shoulder_shag_l.south|shoulders.south",
    "coplanar-overlap:shoulder_shag_r.south|shoulders.south"
  ],
  "ostrich": [
    "coplanar-overlap:beak_tip.down|beak.down"
  ],
  "pangolin": [
    "coplanar-overlap:body.east|scale_plate_2.east",
    "coplanar-overlap:body.west|scale_plate_2.west"
  ],
  "parrot": [
    "coplanar-overlap:tail_l.down|tail_r.down",
    "coplanar-overlap:tail_l.up|tail_r.up"
  ],
  "peacock": [
    "coplanar-overlap:tail_feather_1.north|tail_feather_2.north",
    "coplanar-overlap:tail_feather_1.south|tail_feather_2.south",
    "coplanar-overlap:tail_feather_2.north|tail_feather_3.north",
    "coplanar-overlap:tail_feather_2.south|tail_feather_3.south",
    "coplanar-overlap:tail_feather_3.north|tail_feather_4.north",
    "coplanar-overlap:tail_feather_3.south|tail_feather_4.south",
    "coplanar-overlap:tail_feather_4.north|tail_feather_5.north",
    "coplanar-overlap:tail_feather_4.south|tail_feather_5.south",
    "coplanar-overlap:tail_feather_5.north|tail_feather_6.north",
    "coplanar-overlap:tail_feather_5.south|tail_feather_6.south",
    "coplanar-overlap:tail_feather_6.north|tail_feather_7.north",
    "coplanar-overlap:tail_feather_6.south|tail_feather_7.south"
  ],
  "penguin": [
    "coplanar-overlap:body.north|head.north"
  ],
  "pigeon": [
    "coplanar-overlap:tail_c.down|tail_l.down",
    "coplanar-overlap:tail_c.down|tail_r.down",
    "coplanar-overlap:tail_c.up|tail_l.up",
    "coplanar-overlap:tail_c.up|tail_r.up",
    "coplanar-overlap:wing_l.down|wing_tip_l.down",
    "coplanar-overlap:wing_r.down|wing_tip_r.down"
  ],
  "player": [
    "coplanar-overlap:foot_l.south|leg_l.south",
    "coplanar-overlap:foot_r.south|leg_r.south"
  ],
  "polar_bear": [
    "coplanar-overlap:body.down|rump.down"
  ],
  "porcupine": [
    "coplanar-overlap:leg_bl.south|paw_bl.south",
    "coplanar-overlap:leg_br.south|paw_br.south",
    "coplanar-overlap:leg_fl.south|paw_fl.south",
    "coplanar-overlap:leg_fr.south|paw_fr.south",
    "coplanar-overlap:tail_tip.down|tail.down"
  ],
  "rabbit": [
    "coplanar-overlap:leg_fl.south|paw_fl.south",
    "coplanar-overlap:leg_fr.south|paw_fr.south"
  ],
  "raccoon": [
    "coplanar-overlap:head.down|muzzle.down"
  ],
  "raven": [
    "coplanar-overlap:breast.east|head.east",
    "coplanar-overlap:breast.west|head.west",
    "coplanar-overlap:tail_c.down|tail_l.down",
    "coplanar-overlap:tail_c.down|tail_r.down",
    "coplanar-overlap:tail_c.up|tail_l.up",
    "coplanar-overlap:tail_c.up|tail_r.up",
    "coplanar-overlap:wing_l.down|wing_tip_l.down",
    "coplanar-overlap:wing_r.down|wing_tip_r.down"
  ],
  "red_squirrel": [
    "coplanar-overlap:foot_fl.south|leg_fl.south",
    "coplanar-overlap:foot_fr.south|leg_fr.south"
  ],
  "rhinoceros": [
    "coplanar-overlap:body.north|shoulders.north"
  ],
  "ring_tailed_lemur": [
    "coplanar-overlap:tail_1.east|tail_2.east",
    "coplanar-overlap:tail_1.west|tail_2.west",
    "coplanar-overlap:tail_2.east|tail_3.east",
    "coplanar-overlap:tail_2.west|tail_3.west"
  ],
  "roly_poly": [
    "coplanar-overlap:head.east|tail_plate.east",
    "coplanar-overlap:head.west|tail_plate.west",
    "coplanar-overlap:segment_6.down|segment_7.down"
  ],
  "sloth": [
    "coplanar-overlap:foot_l.south|lower_leg_l.south",
    "coplanar-overlap:foot_r.south|lower_leg_r.south"
  ],
  "snail": [
    "coplanar-overlap:foot.down|shell.down"
  ],
  "spotted_hyena": [
    "coplanar-overlap:leg_bl.south|paw_bl.south",
    "coplanar-overlap:leg_br.south|paw_br.south",
    "coplanar-overlap:leg_fl.south|paw_fl.south",
    "coplanar-overlap:leg_fr.south|paw_fr.south"
  ],
  "starfish": [
    "coplanar-overlap:arm_1_root.down|arm_2_root.down",
    "coplanar-overlap:arm_2_mid.down|arm_2_root.down",
    "coplanar-overlap:arm_2_root.down|arm_3_root.down",
    "coplanar-overlap:arm_3_root.down|arm_4_root.down",
    "coplanar-overlap:arm_4_mid.down|arm_4_root.down",
    "coplanar-overlap:arm_4_root.down|arm_5_root.down"
  ],
  "toucan": [
    "coplanar-overlap:bill_base.down|bill_mid.down",
    "coplanar-overlap:foot_l.north|leg_l.north",
    "coplanar-overlap:foot_r.north|leg_r.north"
  ],
  "upright_bear": [
    "coplanar-overlap:arm_l.south|paw_l.south",
    "coplanar-overlap:arm_r.south|paw_r.south",
    "coplanar-overlap:torso.south|vest_panel.south"
  ],
  "wildebeest": [
    "coplanar-overlap:hoof_bl.south|leg_bl.south",
    "coplanar-overlap:hoof_br.south|leg_br.south",
    "coplanar-overlap:hoof_fl.south|leg_fl.south",
    "coplanar-overlap:hoof_fr.south|leg_fr.south",
    "coplanar-overlap:mane.south|shoulder_hump.south"
  ]
};
