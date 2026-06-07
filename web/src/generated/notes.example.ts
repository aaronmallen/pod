import type { Note } from '../types';

export const NOTICE: string = 'Heads up — Pod 0.5.0 is a complete rewrite of the app and is not backwards compatible with earlier versions. Updating clears your existing local Pod data and starts fresh, so the first time you open it you\'ll need to sign in and re-authorize all of your characters again.';

export const NOTES: Note[] = [
  { tag: 'NEW', tone: 'plasma',  text: 'In-app auto-update: Pod checks for new versions on startup and every 4 hours. A dismissible banner lets you download, install, and restart in one click — no manual download required.' },
  { tag: 'FIX', tone: 'success', text: 'Space Grotesk now renders correctly on Windows. Corrupted font files were causing all UI text to fall back to a symbol font.' },
  { tag: 'FIX', tone: 'success', text: 'App no longer opens a console window on Windows at startup. Previously closing that window would silently terminate Pod.' },
  { tag: 'FIX', tone: 'success', text: 'Splash transition no longer closes the app on Wayland / KDE Plasma 6. The main window now opens fresh with the correct settings instead of mutating the splash surface.' },
];
