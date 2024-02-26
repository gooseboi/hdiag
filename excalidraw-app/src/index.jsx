import React from 'react';
import ReactDOM from 'react-dom/client';
import { Excalidraw } from '@excalidraw/excalidraw';

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
	<div style={{ height: "500px" }}>
		<Excalidraw />
    </div>
  </React.StrictMode>,
);
