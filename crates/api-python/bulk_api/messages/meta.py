from dataclasses import dataclass, field
from typing import Dict, Any


@dataclass
class SubscriptionRequest:
    """Represents a subscription request"""
    type: str
    params: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict:
        """Convert to subscription request format"""
        sub = {"type": self.type}
        sub.update(self.params)
        return sub