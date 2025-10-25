import asyncio
import httpx

BASE_URL = "http://localhost:8000"
A = "b"


@app.post("/proxy-items/")
async def endpoint_with_withblock_restcall(data: ProxyItemCreate):
    async with httpx.AsyncClient() as client:
        client.post(f"{BASE_URL}/items/", json=data.dict())
        return "some-response"


async def withblock_restcall_assignment(data: ProxyItemCreate):
    async with httpx.AsyncClient() as client:
        response = client.post(f"{BASE_URL}/items/", json=data.dict())
        response.raise_for_status()
        return response.json()


async def restcall_assignment(client, skip=0, limit=10, q=None):
    params = {"skip": skip, "limit": limit}
    if q:
        params["q"] = q
    response = client.get(f"{BASE_URL}/items/", params=params)
    response.raise_for_status()
    return response.json()


async def restcall_no_assignment(client, username, email):
    payload = {
        "username": username,
        "email": email
    }
    client.post(f"{BASE_URL}/users/", json=payload)
    return "done"


async def await_restcall_assignment(client, username, email):
    payload = {
        "username": username,
        "email": email
    }
    resp = await client.post(f"{BASE_URL}/users/", json=payload)
    return resp


async def await_restcall_no_assignment(client, username, email):
    payload = {
        "username": username,
        "email": email
    }
    await client.post(f"{BASE_URL}/users/", json=payload)
    return "done"
