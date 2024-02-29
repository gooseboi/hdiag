import { exportToSvg, loadFromBlob } from "@excalidraw/excalidraw";

window.onload = async function main() {
  const input = await (await fetch("/input.excalidraw")).blob();
  const opts = await (await fetch("/export_opts")).json();
  const scene = await loadFromBlob(input, null, null);

  const appState = scene.appState;
  appState.exportBackground = opts.exportBackground;
  appState.exportEmbedScene = opts.exportEmbedScene;
  appState.exportWithDarkMode = opts.exportWithDarkMode;
  appState.exportScale = opts.exportScale;
  const svg = await exportToSvg({
    elements: scene.elements,
    appState: appState,
    files: scene.files,
  });

  const serializer = new XMLSerializer();
  const svgMarkup = serializer.serializeToString(svg);

  fetch("/return", {
	method: "POST",
	body: svgMarkup
  })
};
