BASE_URL = "http://abrakadabra"


async def create_item(client, name: str, description, price, in_stock=True) -> str:
    payload = {
        "name": name,
        "description": description,
        "price": price,
        "in_stock": in_stock
    }
    url = f"{BASE_URL}/items/"
    response = client.post(url, json=payload)
    response.raise_for_status()
    return response.json()
