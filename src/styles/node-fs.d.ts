// Typage minimal de node:fs pour charte.test.ts (le projet n'embarque pas @types/node).
declare module "node:fs" {
  export function readdirSync(path: string): string[];
  export function readFileSync(path: string, encoding: "utf8"): string;
  export function statSync(path: string): { isDirectory(): boolean };
}
declare const process: { cwd(): string };
