import asyncio
import httpx

BASE_URL = "http://localhost:8000"
A = "b"


async def create_item(client, name, description, price, in_stock=True):
    payload = {
        "name": name,
        "description": description,
        "price": price,
        "in_stock": in_stock
    }
    response = client.post(f"{BASE_URL}/items/", json=payload)
    response.raise_for_status()
    return response.json()


async def get_items(client, skip=0, limit=10, q=None):
    params = {"skip": skip, "limit": limit}
    if q:
        params["q"] = q
    response = await client.get(f"{BASE_URL}/items/", params=params)
    response.raise_for_status()
    return response.json()


async def get_item_by_id(client, item_id):
    response = await client.get(f"{BASE_URL}/items/{item_id}")
    if response.status_code == 404:
        return None
    response.raise_for_status()
    return response.json()


async def create_user(client, username, email):
    payload = {
        "username": username,
        "email": email
    }
    response = await client.post(f"{BASE_URL}/users/", json=payload)
    response.raise_for_status()
    return response.json()


async def get_users(client, limit=10):
    response = await client.get(f"{BASE_URL}/users/", params={"limit": limit})
    response.raise_for_status()
    return response.json()


async def search(client, query):
    response = await client.get(f"{BASE_URL}/search/", params={"q": query})
    response.raise_for_status()
    return response.json()


async def main():
    async with httpx.AsyncClient() as client:
        # Create sample item
        item = await create_item(client, "Laptop", "Gaming laptop", 1499.99)
        print("Created item:", item)

        # Read item by ID
        item_data = await get_item_by_id(client, item["id"])
        print("Fetched item by ID:", item_data)

        # List items
        items = await get_items(client)
        print("All items:", items)

        # Create a user
        user = await create_user(client, "johndoe", "john@example.com")
        print("Created user:", user)

        # List users
        users = await get_users(client)
        print("All users:", users)

        # Search for keyword
        results = await search(client, "john")
        print("Search results:", results)


if __name__ == "__main__":
    asyncio.run(main())
