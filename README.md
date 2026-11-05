# zombie-hunt

A swarm of hunter agents stalks a cluster for zombie workloads, idle services, forgotten namespaces, and load balancers serving nobody, then proves each kill before it fires.

## What this is

Cost tools monitor and recommend, and the recommendations die unapplied while production clusters stay heavily over provisioned. Zombie-hunt closes the loop in the open, propose, defend, quarantine, verify, and report. It starts from a reversible quarantine, so the first false positive does not end trust in the hunt. Every kill carries the proof that the workload served nobody, gathered before the trigger is pulled.

## Status

Round 5 is built and in review. It ports zombie-hunt to a native Rust crate with no change to the contracted behaviour, proven against the Python implementation it replaces over the committed examples, the committed fixtures, a generated command line corpus, and differential runs over the snapshot shapes, the separator placements, the path spellings, and the fixture reader edges. The Python implementation is removed in the same slice, and the port stands stated under contract obligations ZH-25 to ZH-35, with probe corrections recorded against the earlier readings of the snapshot cannot read line, the fixture reader nesting ceiling, and the snapshot number digit limit. Round 4 landed in PR 9 and is recorded closed here. The scan and the quarantine plan over one recorded cluster snapshot are in place under contract obligations ZH-1 to ZH-24, classifying every entry by three mechanical proof rules over the recorded signal, owner, and in-flight evidence, listing every suspect and proven zombie with its evidence, and writing a reversible quarantine plan for one proven zombie while executing nothing, and the plan now carries the offline kill call over the completed proof pack, recording a verdict from a trusted recorded judgment and advancing the plan for operator approval only on a trusted quarantine answer, and the holds list now reads as a set of tokens, each token recorded once at parse time however many times the list writes it, and the observation report now reads the outcome of the plan's watch over a later snapshot, reporting the watch still running inside the observation window or a verdict that the quarantine held or broke on the recorded signal and owner facts alone, while executing nothing. Closed rounds 4 of 100. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
