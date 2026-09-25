import init, {
	start_options,
	dump_all,
	run_harness,
	wipe_harness,
} from "./pkg/nommetric.js";

await init({ module_or_path: "./pkg/nommetric_bg.wasm" });

// Temporary — delete when ui/options.rs lands. Module scope is not global, so
// these are attached explicitly to be callable from the devtools console.
Object.assign(globalThis, { dump_all, run_harness, wipe_harness });

start_options();
