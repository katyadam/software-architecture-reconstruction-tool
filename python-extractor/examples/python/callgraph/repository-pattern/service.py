from typing import List, Optional
from user import User
from repository import UserRepository


class UserService:
    def __init__(self, repository: UserRepository):
        self.repository = repository

    def create_user(self, name: str, email: str) -> User:
        # Simple business rule: email must be unique
        existing = [u for u in self.repository.get_all() if u.email == email]
        if existing:
            raise ValueError(f"User with email {email} already exists")

        new_user = User(id=len(self.repository.get_all()) +
                        1, name=name, email=email)
        return self.repository.save(new_user)

    def get_user(self, user_id: int) -> Optional[User]:
        return self.repository.get_by_id(user_id)

    def list_users(self) -> List[User]:
        return self.repository.get_all()

    def delete_user(self, user_id: int) -> bool:
        return self.repository.delete(user_id)
