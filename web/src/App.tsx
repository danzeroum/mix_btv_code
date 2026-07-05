import { AppProvider } from './state/AppContext'
import { ToastProvider } from './components/primitives/Toast'
import { Shell } from './components/shell/Shell'

function App() {
  return (
    <AppProvider>
      <ToastProvider>
        <Shell />
      </ToastProvider>
    </AppProvider>
  )
}

export default App
