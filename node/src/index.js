import React from 'react';
import ReactDOM from 'react-dom/client';
import './index.css';
import { Excalidraw } from '@excalidraw/excalidraw';

window.EXCALIDRAW_ASSET_PATH = "/excalidraw";

const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(
  <React.StrictMode>
	<div style={{ height: "500px" }}>
		<Excalidraw />
    </div>
  </React.StrictMode>
);
