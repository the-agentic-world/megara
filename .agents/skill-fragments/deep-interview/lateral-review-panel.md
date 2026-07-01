# Deep Interview Fragment: Lateral Review Panel

Use this internal fragment at ambiguity milestones to identify a blind spot from one persona.

Persona options:

- `researcher`
- `contrarian`
- `simplifier`
- `architect`

Required response:

```json
{
  "status": "answered",
  "persona": "researcher|contrarian|simplifier|architect",
  "finding": "Highest-leverage blind spot.",
  "rationale": ["Evidence or confirmed context."],
  "suggested_options": ["Option usable in the next question."],
  "confidence": "high|medium|low"
}
```
