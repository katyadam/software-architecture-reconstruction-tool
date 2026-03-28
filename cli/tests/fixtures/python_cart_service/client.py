import httpx


async def fetch_products(client):
    response = await client.get("http://product-service:8000/api/v1/products/")
    return response.json()


async def create_product(client, name, price):
    payload = {"name": name, "price": price}
    response = await client.post("http://product-service:8000/api/v1/products/", json=payload)
    return response.json()
