from typing import List, Optional
from user import User


class UserRepository:
    def __init__(self):
        self._users: List[User] = []

    def get_all(self) -> List[User]:
        return self._users

    def get_by_id(self, user_id: int) -> Optional[User]:
        return next((u for u in self._users if u.id == user_id), None)

    def save(self, user: User) -> User:
        self._users.append(user)
        return user

    def delete(self, user_id: int) -> bool:
        user = self.get_by_id(user_id)
        if user:
            self._users.remove(user)
            return True
        return False
