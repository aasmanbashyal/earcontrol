import React from 'react';
import ReactDOM from 'react-dom/client';
import { Practice } from './Practice';
import { applyTheme, readLiveTheme } from './theme';
import './styles.css';
import './practice.css';

try {applyTheme(readLiveTheme(localStorage),matchMedia('(prefers-color-scheme: dark)').matches);} catch {/* Use the default theme if storage is unavailable. */}

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><Practice/></React.StrictMode>);
