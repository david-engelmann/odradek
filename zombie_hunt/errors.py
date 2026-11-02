"""Error types for zombie-hunt."""


class ZombieHuntError(Exception):
    """Base class for every refusal zombie-hunt raises."""


class SnapshotError(ZombieHuntError):
    """A recorded cluster snapshot is malformed."""


class PlanError(ZombieHuntError):
    """A quarantine plan cannot be written for the named entry."""
