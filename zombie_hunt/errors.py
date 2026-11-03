"""Error types for zombie-hunt."""


class ZombieHuntError(Exception):
    """Base class for every refusal zombie-hunt raises."""


class SnapshotError(ZombieHuntError):
    """A recorded cluster snapshot is malformed."""


class PlanError(ZombieHuntError):
    """A quarantine plan cannot be written for the named entry."""


class ObserveError(ZombieHuntError):
    """An observation report cannot be written for the named entry."""


class JudgmentError(ZombieHuntError):
    """A recorded kill call fixture is outside the recorded shape."""
