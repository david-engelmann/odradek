# Roadmap - zombie-hunt

Zombie-hunt stalks a cluster for workloads that serve nobody, proves each kill before it fires, and starts from a reversible quarantine. The setup slice is landed and the contract stub stands at ZH-S1 to ZH-S5. Round 1 builds the scan over one recorded cluster snapshot, the suspect list with evidence, and a reversible quarantine plan for one proven zombie.

## Decision models in zombie-hunt

Judgment enters at the kill call, and only there.

Everything before the call is evidence gathering and stays deterministic. The scan reads recorded cluster state, the suspect list is built from rules about traffic, ownership, and age, and the proof pack for a suspect is assembled from recorded facts. No model gathers evidence and no model writes the proof.

When a proof pack is complete, one judgment remains. Given this evidence, does the workload serve nobody, or is there a reading of the same facts under which something real depends on it. That judgment is a candidate for a decision model call shaped as a choice over three labels, quarantine, watch, and spare. The evidence pack is the state the call reads, and the labels are fixed by the contract.

What the model decides is bounded by two rules that outrank it. The first action on any proven zombie is a reversible quarantine, and that rule is deterministic and absolute, so the strongest action a model answer can support is still reversible. And in the early slices every hunt works from recorded or synthetic state, so a quarantine answer is exercised against recorded state and a plan, never against a live cluster. A model abstention, a watch answer, or a spare answer all end the same way, with no action taken. Only a quarantine answer with confidence above the threshold set in the contract advances the plan, and advancing a plan against anything real remains an operator decision that waits for approval through the program.

What stays deterministic is the safety shape of the whole repo. Proof before any action, reversible first, recorded state only in the early slices, and no code path that connects to a live cluster are rules with no model inside them.

The candidate model is Typesafe's Jev, called through the program's Typesafe connector, with the OpenAI decision model as the comparison point. Both are evaluated against the same recorded fixtures before either is adopted at the decision point, and the deterministic core of this repo never depends on a provider being reachable.

Tests stay offline. Live answers are captured once, in a bounded recording session, as request and response fixture pairs built from synthetic proof packs, and the suite plays those fixtures back through a fixture player. No test calls the network. A schema test pins the fixture shape so a provider response that changes shape fails loudly in the recording session instead of silently in production.

The live-call budget proposed for this repo is an envelope, not an open account. Trivial verification is capped at 5 calls. Fixture recording is capped at 150 calls in one designated session per round that needs new fixtures. A comparison evaluation is capped at 300 calls per model when a decision point is being adopted. Every call carries synthetic state only. Anything beyond the envelope waits for approval through the program's main chat.
