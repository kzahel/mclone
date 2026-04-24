export const RuleTestType = {
  BLOCK_TEST: "block_match",
  BLOCKSTATE_TEST: "blockstate_match",
  TAG_TEST: "tag_match",
} as const;

export type RuleTestType = (typeof RuleTestType)[keyof typeof RuleTestType];
