import React from "react";
import ReactDOM from "react-dom/client";
import { exportToSvg, loadFromBlob } from "@excalidraw/excalidraw";

window.onload = async function main() {
  const input = await (await fetch("/input.excalidraw")).blob();
  const scene = await loadFromBlob(input, null, null);

  const svg = await exportToSvg({
    elements: scene.elements,
    appState: scene.appState,
    files: scene.files,
  });

  const serializer = new XMLSerializer();
  const svgMarkup = serializer.serializeToString(svg);

  fetch("/return", {
	method: "POST",
	body: svgMarkup
  })
};
