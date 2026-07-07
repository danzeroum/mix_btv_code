import { useEffect } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchSandbox } from '../../../api/sandbox'
import { fetchSkills } from '../../../api/skills'

export function Sandbox() {
  const sandboxState = useAsyncAction(fetchSandbox)
  const skillsState = useAsyncAction(fetchSkills)

  useEffect(() => {
    void sandboxState.run()
    void skillsState.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="stack">
      <AsyncStatus state={sandboxState.state} onRetry={() => void sandboxState.run()}>
        {({ profile, ping }) => (
          <Card>
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <strong>Perfil de confinamento</strong>
              <Badge color={ping ? 'var(--ok)' : 'var(--red)'}>
                daemon Docker {ping ? 'conectado' : 'indisponível'}
              </Badge>
            </div>
            <div className="stack" style={{ marginTop: 8, fontSize: 12 }}>
              <div>
                imagem: <span className="mono">{profile.image}</span>
              </div>
              <div>rede: {profile.network_disabled ? 'desabilitada' : 'habilitada'}</div>
              <div>memória: {profile.mem_limit_mb} MB</div>
              <div>cpu quota: {profile.cpu_quota}</div>
              <div>timeout: {profile.timeout_secs}s</div>
              <div>rootfs: {profile.rootfs_readonly ? 'read-only' : 'read-write'}</div>
              <div>capabilities: {profile.cap_drop_all ? 'cap-drop ALL' : 'padrão'}</div>
              <div>privilégios: {profile.no_new_privileges ? 'no-new-privileges' : 'padrão'}</div>
            </div>
            <div style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
              rootfs/capabilities/privilégios são constantes hardcoded de <span className="mono">run_with</span>,
              não campos configuráveis.
              {!ping && ' Sem daemon: fail-closed — nenhuma skill de terceiro roda até ele responder.'}
            </div>
          </Card>
        )}
      </AsyncStatus>

      <Card>
        <strong>Skills de terceiro</strong>
        <div style={{ marginTop: 8 }}>
          <AsyncStatus state={skillsState.state} onRetry={() => void skillsState.run()}>
            {(skills) => {
              const thirdParty = skills.filter((s) => s.source === 'third-party')
              return thirdParty.length === 0 ? (
                <div style={{ fontSize: 12, color: 'var(--faint)' }}>
                  nenhuma skill de terceiro em <span className="mono">.forge/skills/</span>.
                </div>
              ) : (
                <Table
                  rowKey={(s) => s.id}
                  rows={thirdParty}
                  columns={[
                    { key: 'id', header: 'skill', render: (s) => s.id },
                    { key: 'status', header: 'status', render: (s) => s.status },
                    { key: 'detail', header: 'detalhe', render: (s) => s.detail },
                  ]}
                />
              )
            }}
          </AsyncStatus>
        </div>
        <div style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
          tela read-only — instalar/vetar/habilitar/remover uma skill de terceiro pelo navegador fica fora desta
          fase.
        </div>
      </Card>
    </div>
  )
}
