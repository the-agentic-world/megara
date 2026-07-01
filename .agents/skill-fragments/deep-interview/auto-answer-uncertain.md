# Deep Interview Fragment: Auto Answer Uncertain

Use this internal fragment when the user explicitly asks the agent to choose, opts out, or gives an uncertain answer.

Return a conservative assumption that keeps the interview moving without pretending certainty.

Required response:

```json
{
  "status": "answered",
  "answer": "One concise assumption.",
  "rationale": ["Evidence or confirmed context."],
  "confidence": "high|medium|low",
  "uncertainty": "Remaining uncertainty or null."
}
```
