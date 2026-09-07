import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { applyPreferences, readPreferences } from './state/store'
import './styles/index.css'

// Before the first render, not in an effect.
//
// The stylesheet's default is the dark theme, so a light-theme user whose
// preference is applied after mounting sees the window painted dark and then
// corrected — and the parts of the chrome that do not repaint immediately can
// stay dark for a frame or two. Setting the attribute first means the first
// paint is already the right one.
applyPreferences(readPreferences())

const container = document.getElementById('root')
if (!container) throw new Error('the root element is missing from index.html')

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

