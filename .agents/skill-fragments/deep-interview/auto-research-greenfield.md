# Deep Interview Fragment: Auto Research Greenfield

Use this internal fragment when a greenfield interview question needs candidate answers before asking the user.

Return two or three ranked candidates grounded in confirmed constraints.

Required response:

```json
{
  "status": "answered",
  "candidates": [
    {
      "rank": 1,
      "answer": "Candidate answer.",
      "rationale": "Why it fits.",
      "risks_or_tradeoffs": "Main risk.",
      "confidence": "high|medium|low"
    }
  ],
  "recommendation": "Strongest candidate and why.",
  "follow_up_gap": "Remaining uncertainty."
}
```
