import tseslint from "typescript-eslint";
import solid from "eslint-plugin-solid/configs/typescript";

export default tseslint.config(
  { ignores: ["dist/", "coverage/", "node_modules/", "src/bindings.ts"] },
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    ...solid,
  },
);
