//! Utilitário de teste compartilhado entre os módulos que mutam
//! `std::env::current_dir` (`web_agent`, `squad_agent`) — um lock GLOBAL
//! (não um por módulo) evita que testes de módulos diferentes corram
//! concorrentemente e pisem um no CWD do outro (`cargo test` roda testes em
//! threads do MESMO processo por padrão). `tokio::sync::Mutex` (não
//! `std::sync`) porque o guard fica retido através de `.await` nos testes
//! que o usam — um `std::sync::MutexGuard` preso assim é o lint
//! `clippy::await_holding_lock`.
#![cfg(test)]

pub(crate) static CWD_GUARD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) async fn lock_cwd() -> tokio::sync::MutexGuard<'static, ()> {
    CWD_GUARD.lock().await
}
