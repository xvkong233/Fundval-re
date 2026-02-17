import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    rules: {
      // 当前代码库仍存在较多渐进迁移的 `any`，先允许通过 lint；后续逐步收敛类型后再收紧。
      "@typescript-eslint/no-explicit-any": "off",
      // 该规则对“初始化 localStorage -> setState”场景误报较多，先关闭以避免噪音。
      "react-hooks/set-state-in-effect": "off",
    },
  },
  // Override default ignores of eslint-config-next.
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
  ]),
]);

export default eslintConfig;
