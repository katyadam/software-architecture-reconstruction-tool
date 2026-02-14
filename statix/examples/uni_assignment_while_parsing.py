from collections import namedtuple

from fastapi import HTTPException

from clinical_data_service.singletons import logger
from clinical_data_service.slide_instance import SlideInstance
from clinical_data_service.utils.slide_storage_utils import validate_slide_storage_address
from clinical_data_service.zmq_connection import PluginConnection


class MissingPluginConfigurationError(Exception):
    pass


class InvalidPluginHostError(Exception):
    pass


PluginData = namedtuple("PluginData", ["name", "host", "port", "min_workers"])


class SlideManager:
    def __init__(self, data_dir: str, plugin_addresses: str):
        self._data_dir = data_dir
        self._parsed_plugins = self._parse_plugins(
            plugin_addresses=plugin_addresses)
        self._slide_cache = None
        self._connections = None

    async def initialize_plugins(self):
        self._slide_cache = {}
        self._connections = {}
        plugin_data: PluginData
        for plugin_data in self._parsed_plugins:
            connection: PluginConnection
            try:
                connection = PluginConnection(
                    name=plugin_data.name,
                    host=plugin_data.host,
                    port=plugin_data.port,
                    min_workers=plugin_data.min_workers,
                )
                if not await connection.is_alive():
                    connection.close()
                    logger.warning(
                        "Plugin %s does not seem to be alive. Plugin was not added to available plugins.",
                        plugin_data.name,
                    )
                    continue
            except Exception as e:
                logger.error(e)
                continue
            connection.initialize()
            self._connections[plugin_data.name] = connection
            logger.info(
                "Plugin %s is alive. Plugin was added to available plugins.", plugin_data.name)

    async def get_slide_handle(self, slide_filepath: str, plugin=None) -> SlideInstance:
        val_slide_filepath = validate_slide_storage_address(
            slide_file=slide_filepath, data_dir=self._data_dir)

        # use specific plugin
        if plugin:
            plugin_name = next(
                (p for p in self._connections.keys() if (plugin in p)), None)
            if plugin_name:
                connection: PluginConnection = self._connections.get(
                    plugin_name)
                if not connection:
                    plugin_names = [
                        plugin_data.name for plugin_data in self._parsed_plugins]
                    raise HTTPException(
                        status_code=500,
                        detail=f"WSI plugin {plugin_name} not available. \
                            Available plugins: {plugin_names}",
                    )

                return SlideInstance(filepath=val_slide_filepath, connection=connection)
            else:
                raise HTTPException(
                    status_code=500, detail=f"Selected plugin {plugin} not in {list(self._connections.keys())}"
                )

        # get suitable plugin from cache
        slide_handle = self._slide_cache.get(val_slide_filepath)
        if slide_handle:
            return slide_handle
        logger.info(self._connections.items())
        # add suitable plugin to cache and return
        for plugin_name, connection in self._connections.items():
            logger.info(plugin_name)
            slide_handle = SlideInstance(
                filepath=val_slide_filepath, connection=connection)
            if await slide_handle.is_supported():
                self._slide_cache[val_slide_filepath] = slide_handle
                return slide_handle

        raise HTTPException(
            status_code=404, detail="Slide format is not supported by any plugin")

    def _parse_plugins(self, plugin_addresses: str) -> list:
        plugins = plugin_addresses.split("|")
        results = []
        for plugin in plugins:
            split_semicolon = plugin.split(";")
            name, min_workers = split_semicolon[0], None
            if len(split_semicolon) > 1:
                min_workers = int(split_semicolon[1])
            split_colon = name.split(":")
            host, port = split_colon[0], None
            if len(split_colon) > 1:
                port = split_colon[1]
            plugin_data = PluginData(
                name=name, host=host, port=port, min_workers=min_workers)
            results.append(plugin_data)

        if not results:
            raise MissingPluginConfigurationError(
                "No valid plugin specified in the application settings")
        return results
