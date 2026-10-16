---
name: safe-shutdown
description: Desligamento ou reinicialização segura do computador. Fecha graciosamente aplicativos e janelas abertas antes de disparar o comando de shutdown do sistema operacional.
---

# 🛑 Safe Shutdown Skill (`safe-shutdown`)

> **Caminhos**: `<skills_root>` é a pasta que contém esta skill (o diretório pai da pasta deste `SKILL.md`). Use o caminho absoluto real ao executar os comandos.

> [!CAUTION]
> **REGRA DE SEGURANÇA CRÍTICA E INVIOLÁVEL**:
> NUNCA, sob hipótese alguma, desligar, reiniciar ou agendar o desligamento/reinicialização do computador sem uma ordem DIRETA e EXPLÍCITA do usuário durante a conversa atual (ex: *"desligue o computador"*, *"reinicie o pc"*).
> É TERMINANTEMENTE PROIBIDO executar esta rotina de forma autônoma, proativa ou em decorrência de conclusão de builds, testes, deploys, releases ou atualizações de software.

Esta skill fornece uma rotina segura e controlada para fechar janelas e aplicativos do usuário de forma graciosa e desligar ou reiniciar o computador Windows, EXCLUSIVAMENTE quando expressamente ordenado pelo usuário.

---

## 📋 Comportamento & Procedimento

1. **🧠 Salvamento da Sessão (pelo agente)**: Antes de rodar o script, se a skill `session-memory` estiver disponível, o agente escreve o Master Summary atualizado do projeto e executa `<skills_root>/session-memory/finalize_session.py --project <projeto> --file <sumario.md>`. O script de desligamento não consolida a memória sozinho.
2. **🔍 Identificação de Janelas Abertas**: Localiza processos com janelas ativas (`MainWindowHandle != 0`), ignorando terminais essenciais em execução.
3. **🚪 Fechamento Gracioso**: Envia `CloseMainWindow()` para que os aplicativos salvem dados antes de fechar.
4. **⚡ Desligamento Seguro**: Dispara o comando nativo `shutdown /s /t 10 /c "AnyContext Safe Shutdown"`.
5. **🛑 Cancelamento**: Permite abortar o desligamento a qualquer momento com `shutdown /a`.

---

## 🛠️ Scripts & Utilitários

### 1. Script PowerShell (`safe_shutdown.ps1`)
Localizado em: `<skills_root>/safe-shutdown/safe_shutdown.ps1`

```powershell
powershell -ExecutionPolicy Bypass -File "<skills_root>/safe-shutdown/safe_shutdown.ps1"
```

### 2. Parâmetros Suportados:
- `-DelaySeconds 10`: Tempo de contagem regressiva antes de desligar (padrão: 10s).
- `-Restart`: Reinicia o computador (`shutdown /r`) em vez de desligar.
- `-Abort`: Aborta qualquer desligamento programado (`shutdown /a`).
