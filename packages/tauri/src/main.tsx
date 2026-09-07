import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './shell-theme.css';
import { Root } from './Root';

// Stamp the theme before React's first render so the welcome screen never
// flashes the light palette on a dark system — the entry module runs before
// any paint of #root's contents.
const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root container #root not found');
}

createRoot(container).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
