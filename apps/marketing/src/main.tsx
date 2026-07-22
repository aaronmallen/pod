import { StrictMode } from 'react';
import { hydrateRoot } from 'react-dom/client';
import { App } from './App';

const rootEl = document.getElementById('root');
if (!rootEl) throw new Error('Root element not found');

hydrateRoot(
  rootEl,
  <StrictMode>
    <App />
  </StrictMode>,
);
