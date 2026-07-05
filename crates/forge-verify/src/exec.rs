//! Execução de um passo com timeout que mata o GRUPO de processos inteiro,
//! não só o filho direto — a lição da Fase 4d (`forge-sidecar`, o `uv run`
//! que orfanava o Python): um `cargo test`/`sh -c '... &'` que spawna
//! subprocessos deixaria órfãos rodando se só o pid direto fosse morto.

use std::io;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub struct StepResult {
    pub output: io::Result<Output>,
    pub duration_ms: u64,
    pub timed_out: bool,
}

/// Roda `program args...` capturando stdout/stderr. Com `timeout` presente,
/// mata o grupo de processos inteiro (não só o pid direto) se estourar.
pub fn run_with_timeout(program: &str, args: &[String], timeout: Option<Duration>) -> StepResult {
    let start = Instant::now();
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0); // pgid do filho vira o próprio pid — grupo isolado

    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            return StepResult {
                output: Err(e),
                duration_ms: start.elapsed().as_millis() as u64,
                timed_out: false,
            }
        }
    };

    let pid = child.id();
    let handle = thread::spawn(move || child.wait_with_output());

    let Some(deadline_dur) = timeout else {
        let output = join_output(handle);
        return StepResult {
            output,
            duration_ms: start.elapsed().as_millis() as u64,
            timed_out: false,
        };
    };

    let deadline = Instant::now() + deadline_dur;
    let mut timed_out = false;
    loop {
        if handle.is_finished() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            kill_process_group(pid);
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    let output = join_output(handle);
    StepResult {
        output,
        duration_ms: start.elapsed().as_millis() as u64,
        timed_out,
    }
}

fn join_output(handle: thread::JoinHandle<io::Result<Output>>) -> io::Result<Output> {
    handle.join().unwrap_or_else(|_| {
        Err(io::Error::other(
            "thread de espera do processo entrou em pânico",
        ))
    })
}

#[cfg(unix)]
fn kill_process_group(pid: u32) {
    // process_group(0) fez o filho virar líder do próprio grupo (pgid == pid);
    // `-pid` mata o grupo inteiro — filhos que ele próprio tiver gerado inclusive.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_pid: u32) {
    // Sem process_group fora do Unix nesta versão; o Child ainda é derrubado
    // pelo drop ao final do escopo, só não há garantia sobre netos.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roda_ate_o_fim_quando_sem_timeout() {
        let result = run_with_timeout("true", &[], None);
        assert!(!result.timed_out);
        assert_eq!(result.output.unwrap().status.code(), Some(0));
    }

    #[test]
    fn timeout_mata_o_processo_e_marca_timed_out() {
        let start = Instant::now();
        let result = run_with_timeout(
            "sleep",
            &["5".to_string()],
            Some(Duration::from_millis(150)),
        );
        assert!(result.timed_out);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "deveria ter matado bem antes dos 5s do sleep"
        );
        // morto por sinal: status.code() é None em Unix.
        assert!(result.output.unwrap().status.code().is_none());
    }

    #[test]
    fn timeout_mata_o_grupo_inteiro_nao_so_o_processo_direto() {
        // `sh` gera um `sleep` em segundo plano e espera por ele — se só o pid
        // direto (sh) fosse morto, o `sleep` ficaria órfão rodando.
        let script = "sleep 50 & wait".to_string();
        let result = run_with_timeout(
            "sh",
            &["-c".to_string(), script],
            Some(Duration::from_millis(200)),
        );
        assert!(result.timed_out);

        std::thread::sleep(Duration::from_millis(300)); // dá tempo do kernel refletir o kill
        let still_running = Command::new("pgrep")
            .args(["-f", "sleep 50"])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
        assert!(
            !still_running,
            "sleep 50 ficou órfão — o kill não pegou o grupo inteiro"
        );
    }
}
