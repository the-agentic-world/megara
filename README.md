# Megara

[![CI](https://github.com/the-agentic-world/megara/actions/workflows/ci.yml/badge.svg)](https://github.com/the-agentic-world/megara/actions/workflows/ci.yml)
[![Release](https://github.com/the-agentic-world/megara/actions/workflows/release.yml/badge.svg)](https://github.com/the-agentic-world/megara/actions/workflows/release.yml)

## 프로젝트 소개

Megara라는 이름은 메가라 학파에서 가져왔습니다. 메가라 학파는 소크라테스의 제자 에우클레이데스가 세운 학파로, 논리와 변증, 엄밀한 논박을 중시했습니다. 여러 역할의 에이전트가 요구사항, 계획, 실행, 검증을 그냥 이어 붙이는 것이 아니라 서로 따지고 검토하며 더 단단한 결론으로 수렴한다는 이미지와 맞닿아 있습니다.

[가재코드(GJC)](https://github.com/Yeachan-Heo/gajae-code)의 하네스가 보여준 매력은 단순한 프롬프트 묶음이 아니라, 요구사항 인터뷰, 합의 기반 계획, durable goal 실행, 역할 기반 리뷰를 하나의 작업 방식으로 묶는 데 있습니다. Megara는 그 방식을 특정 런타임에 묶어두지 않고 다른 에이전트로 이식하기 쉽게 만들기 위해 인스톨러와 런타임 투영 계층을 분리했습니다.

저장소의 `.agents/` 디렉터리가 내장 하네스의 source of truth입니다. `megara install`은 이 파일들을 선택한 범위에 설치하고, Codex 같은 런타임이 읽을 수 있는 형태로 `.codex/` 또는 `~/.codex/`에 투영합니다.

포함된 워크플로:

- `deep-interview`: 모호한 요구사항을 질문으로 좁혀 실행 가능한 명세로 만듭니다.
- `ralplan`: planner, architect, critic 리뷰를 거쳐 승인 대기 계획을 만듭니다.
- `ultragoal`: 승인된 계획을 durable goal로 쪼개고 검증 증거와 함께 완료합니다.
- `team`: 분리 가능한 작업을 여러 lane과 역할로 나눠 조율합니다.

포함된 역할 에이전트:

- `executor`
- `planner`
- `architect`
- `critic`

내장 기본 활성 스킬:

- `caveman`: [juliusbrussee/caveman](https://github.com/juliusbrussee/caveman)을 Megara에 내장한 짧은 응답 압축 스킬입니다. 별도 설치 없이 하네스와 함께 설치되고, 새 세션과 재개 세션에서 기본 활성화됩니다.

## 설치안내

macOS 최신 릴리스를 설치합니다.

```bash
curl -fsSL https://github.com/the-agentic-world/megara/releases/latest/download/install.sh | sh
```

특정 버전이나 설치 위치를 지정할 수 있습니다.

```bash
curl -fsSL https://github.com/the-agentic-world/megara/releases/latest/download/install.sh | MEGARA_VERSION=v0.1.0 MEGARA_INSTALL_DIR="$HOME/.local/bin" sh
```

설치 스크립트는 macOS arm64와 Intel을 지원하며 기본 설치 위치는 `/usr/local/bin`입니다.

Homebrew로도 설치할 수 있습니다.

```bash
brew install the-agentic-world/tap/megara
```

소스에서 직접 빌드하려면 Rust toolchain이 필요합니다.

```bash
cargo build --release
./target/release/megara --version
```

## 사용법

설치 wizard를 실행합니다.

```bash
megara install
```

현재 프로젝트에 Codex용 하네스를 설치합니다.

```bash
megara install --scope project --target codex
```

전역 범위에 설치합니다.

```bash
megara install --scope global --target codex
```

설치 상태와 drift를 확인합니다.

```bash
megara doctor --scope project --target codex
```

`.agents/` source of truth에서 런타임 파일을 다시 투영합니다.

```bash
megara sync --scope project --target codex
```

지원 대상과 템플릿을 확인합니다.

```bash
megara targets list
megara templates list
```

설치 범위는 두 가지입니다.

- `project`: 현재 프로젝트의 `.agents/`에 SSOT를 쓰고 `.codex/`로 Codex 파일을 투영합니다.
- `global`: `~/.megara`에 SSOT를 쓰고 `~/.codex/`로 Codex 파일을 투영합니다.

Megara는 기본적으로 기존 사용자 파일을 보호합니다. 목적지가 Megara 관리 파일이 아니면 충돌을 보고하고 그대로 둡니다. Megara가 파일 소유권을 가져가야 할 때만 `--force`를 사용하세요.

### 프롬프트로 하네스 사용하기

프로젝트 범위 설치 후에는 해당 프로젝트를 새 Codex 세션으로 열고, 프롬프트에 워크플로 이름을 직접 넣어 사용합니다. Codex App은 세션 시작 시 hook을 읽으므로, 이미 열려 있던 세션에는 방금 설치한 hook이 소급 적용되지 않습니다.

Megara에는 `caveman`이 내장되어 있어 기본 응답이 짧게 압축됩니다. 일반 문체가 필요하면 다음처럼 요청합니다.

```text
normal mode
```

다시 켜거나 강도를 바꿀 때는 다음처럼 요청합니다.

```text
/caveman lite
/caveman full
/caveman ultra
```

요구사항이 아직 흐릿할 때:

```text
$deep-interview --standard "사용자가 자연어로 워크플로를 설치하고 검증하는 경험을 만들고 싶다"
```

구현 전에 합의된 계획만 먼저 받고 싶을 때:

```text
$ralplan --interactive "install.sh 릴리스 smoke test를 더 견고하게 만드는 계획을 세워줘"
```

승인된 계획을 끝까지 실행하게 할 때:

```text
$ultragoal "방금 승인한 계획을 목표로 나눠 구현하고, 각 목표마다 검증 증거를 남겨줘"
```

작업을 여러 lane으로 나눌 때:

```text
$team "승인된 계획을 구현, 검증, 문서 lane으로 나눠 병렬로 진행해줘"
```

일반적인 흐름은 다음과 같습니다.

1. 모호한 아이디어는 `$deep-interview`로 명세화합니다.
2. 구현 전에는 `$ralplan`으로 계획과 리뷰를 승인받습니다.
3. 승인 후에는 `$ultragoal`로 실행과 검증 증거를 남깁니다.
4. 독립적인 작업 흐름이 있으면 `$team`으로 lane을 나눕니다.

## 현재 제약사항

- v1은 Codex만 대상으로 합니다. 다른 에이전트 런타임을 염두에 둔 구조이지만 아직 투영 어댑터는 `codex`만 구현되어 있습니다.
- 릴리스 설치 스크립트는 macOS arm64와 Intel만 지원합니다. Linux와 Windows는 현재 소스 빌드로만 사용할 수 있습니다.
- Codex App은 세션 시작 시 hook을 읽습니다. 프로젝트 범위 설치 후에는 저장된 프로젝트 또는 정확한 설치 디렉터리로 새 세션을 열어야 합니다.
- 프로젝트 없는 Codex App 세션은 `name-2` 같은 sibling 디렉터리를 만들 수 있습니다. 이 경우 설치한 `.agents/`와 `.codex/`가 없는 위치에서 세션이 시작될 수 있습니다.
- `deep-interview`, `ralplan`, `ultragoal`의 상태는 hook과 Megara CLI가 관리합니다. `.agents/state/workflows/**`를 직접 편집해 handoff를 강제로 만들지 마세요.
- 기본 내장 하네스 locale은 `ko-KR`입니다. 파일 경로, 명령어, config key 같은 기술 literal은 그대로 유지됩니다.

## GJC 저장소

Megara는 GJC 하네스가 보여준 작업 방식에서 출발했습니다. 원본 아이디어와 더 큰 실험 맥락이 궁금하다면 [Yeachan-Heo/gajae-code](https://github.com/Yeachan-Heo/gajae-code) 저장소도 함께 살펴보세요.
