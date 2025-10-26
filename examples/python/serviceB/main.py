from fastapi import FastAPI
from pydantic import BaseModel
import httpx

BASE_URL = "http://localhost:8000"

app = FastAPI()


class ProxyItemCreate(BaseModel):
    title: str
    qty: int


@app.post("/proxy-items/")
async def proxy_create_item(data: ProxyItemCreate):
    async with httpx.AsyncClient() as client:
        response = await client.post(f"{BASE_URL}/items/", json=data.dict())
        response.raise_for_status()
        return response.json()
