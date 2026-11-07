import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { applyTheme, readSavedTheme } from './theme';
import './styles.css';

try {applyTheme(readSavedTheme(localStorage),matchMedia('(prefers-color-scheme: dark)').matches);} catch {/* Use the default theme if storage is unavailable. */}

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
