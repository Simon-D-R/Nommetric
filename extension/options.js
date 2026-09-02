// external file, extension_pages CSP is `script-src 'self'`
import init, { start_options } from "./pkg/nommetric.js";

await init({ module_or_path: "./pkg/nommetric_bg.wasm" });
start_options();