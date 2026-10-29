// The two flavors of this deployment's externally managed engine. Both run in the same
// single-process inference engine behind the llama-swap gateway (one DIT at a time);
// picking a flavor asks the service to point at the other one, and the service's
// "switching" status (Loading <model>) tracks the gateway's eviction + load.
//
// Kept in the browser bundle on purpose: this is a local-deployment decision, not an
// upstream one. The service enforces whatever `externalEngine` it is told to use; the
// picker is the human face of it.
import type { ExternalEngine } from "./types";

export const GATEWAY = "http://127.0.0.1:8081/upstream";

export interface Flavor extends ExternalEngine {
  label: string;
  sub: string;
}

export const FLAVORS: Flavor[] = [
  {
    label: "Turbo",
    sub: "8 steps · fastest",
    gateway: GATEWAY,
    model: "kokon-turbo",
    modelName: "qwen-image-2.1-turbo-Q4_K_M.gguf",
    // The turbo schedule: short and unguided.
    sample: { steps: 8, cfg: 1.0, clear_sigmas: true },
  },
  {
    label: "Full",
    sub: "20 steps · most detail",
    gateway: GATEWAY,
    model: "kokon-full",
    modelName: "qwen-image-2.1-Q8_0.gguf",
    // The non-turbo model, guided and longer.
    sample: { steps: 20, cfg: 6.0, sampler: "euler", clear_sigmas: true },
  },
];

/** "Turbo"/"Full" for a configured flavor, its model name otherwise. */
export function flavorLabel(e: ExternalEngine | null | undefined): string {
  if (!e) return "";
  return FLAVORS.find((f) => f.model === e.model)?.label ?? e.model;
}
