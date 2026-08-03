# Question authoring manual review v1

- review_id: `ER-MANUAL-QST-001`
- checklist: `MPC-MAN-001`, `MPC-MAN-002`, `MPC-MAN-003`, `MPC-MAN-004`, `MPC-QST-010`, `MPC-QST-015`
- fixture: `gold-v1.json`
- reviewer: `planning-core-review`
- decision: `PASS`
- signature: `planning-core-review/gold-v1`

판정 근거:

1. `audience`, `context`, `one-decision`, `terms`, `choices`, `impact`, `recommendation` 일곱 규칙을 `rubric-failure-map.json`의 gold 행으로 각각 확인했다.
2. `anti-jargon.json`은 약어를 풀어 썼지만 문맥상 역할·영향을 설명하지 못하므로 `terms`와 `MPC-QST-015` 실패로 연결했다.
3. `anti-label-repeat.json`은 duplicate choice ID와 같은 label·방향·장점·감수할 점을 사용하므로 `choices`와 `MPC-QST-004` 실패로 연결했다.
4. gold의 두 choice는 진행 방향, benefit, tradeoff가 서로 다르고 각 field가 한 번씩 표시된다. recommendation은 `initial_request/request` 근거와 연결되며 선택지 `inspect`에만 지정된다. 근거 원문은 fixture의 `source_material`에 있는 “기존 파일을 먼저 확인하면 같은 결정을 반복하지 않고 현재 진입점을 근거로 삼을 수 있다.”이다.
5. 전문용어 `근거`의 field 자체가 “기존 결정을 반복하지 않게 하는 역할”과 “먼저 확인할 가치”라는 문맥상 역할·영향을 설명하므로 단순한 약어 확장이 아니다.

이 기록은 fixture와 pure projection의 수동 품질 review evidence이며 실제 Codex/Pi host UI confirmation을 대체하지 않는다.
