(async () => {
	console.log("nommetric: boot");

	try {
		await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
		console.log("nommetric: wasm-compile ok");
	} catch (e) {
		console.error("nommetric: wasm-compile blocked", e);
		return;
	}

	const glue = await import(chrome.runtime.getURL("pkg/nommetric.js"));
	console.log("nommetric: dynamic-import ok");

	await glue.default({ module_or_path: chrome.runtime.getURL("pkg/nommetric_bg.wasm") });
	glue.start_content();
})().catch((e) => console.error("nommetric: boot failed", e));