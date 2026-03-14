from typing import List, Optional
from pydantic import BaseModel


class Email(BaseModel):
    username: str
    domain: str


class MailService():
    emails: List[Email]
    emails2: list[Email]
