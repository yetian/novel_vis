/// <reference types="vite/client" />

/** 由 vite.config.ts 注入的应用版本，来源为 package.json。 */
declare const __APP_VERSION__: string;

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, unknown>;
  export default component;
}
