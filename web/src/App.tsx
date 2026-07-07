import { AppProvider } from './state/AppContext'
import { SessionProvider } from './state/SessionContext'
import { ToastProvider } from './components/primitives/Toast'
import { Shell } from './components/shell/Shell'

function App() {
  return (
    <AppProvider>
      <ToastProvider>
        {/* Acima da troca de tela: um pedido de permissão pode chegar
            enquanto o usuário está em Sessão mas precisa sobreviver à
            navegação até a tela Permissão (Fase 7 Onda 2). */}
        <SessionProvider>
          <Shell />
        </SessionProvider>
      </ToastProvider>
    </AppProvider>
  )
}

export default App
