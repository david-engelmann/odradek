# zombie-hunt

A swarm of hunter agents stalks a cluster for zombie workloads, idle services, forgotten namespaces, and load balancers serving nobody, then proves each kill before it fires.

## What this is

Cost tools monitor and recommend, and the recommendations die unapplied while production clusters stay heavily over provisioned. Zombie-hunt closes the loop in the open, propose, defend, quarantine, verify, and report. It starts from a reversible quarantine, so the first false positive does not end trust in the hunt. Every kill carries the proof that the workload served nobody, gathered before the trigger is pulled.

## Status

Round 1 built and in review. The scan and the quarantine plan over one recorded cluster snapshot are in place under contract obligations ZH-1 to ZH-12, classifying every entry by three mechanical proof rules over the recorded signal, owner, and in-flight evidence, listing every suspect and proven zombie with its evidence, and writing a reversible quarantine plan for one proven zombie while executing nothing. Closed rounds 0 of 100. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
