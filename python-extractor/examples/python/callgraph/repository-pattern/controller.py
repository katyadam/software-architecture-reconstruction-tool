from service import UserService


class UserController:
    def __init__(self, service: UserService):
        self.service = service

    def create_user(self, name: str, email: str):
        try:
            user = self.service.create_user(name, email)
            return {"status": "success", "data": user}
        except ValueError as e:
            return {"status": "error", "message": str(e)}

    def get_user(self, user_id: int):
        user = self.service.get_user(user_id)
        if user:
            return {"status": "success", "data": user}
        return {"status": "error", "message": "User not found"}

    def list_users(self):
        return {"status": "success", "data": self.service.list_users()}

    def delete_user(self, user_id: int):
        if self.service.delete_user(user_id):
            return {"status": "success", "message": "User deleted"}
        return {"status": "error", "message": "User not found"}
