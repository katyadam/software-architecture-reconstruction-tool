from aiohttp.client_exceptions import ClientConnectorError, ClientOSError
from fastapi import HTTPException

from ..empaia_sender_auth import CustomHTTPException
from .singletons import as_url, http_client, jes_url, logger, mds_url, organization_id, settings

CLIENT_EXCEPTIONS = (ClientConnectorError, ClientOSError,
                     CustomHTTPException, HTTPException)
GENERIC_ERROR_MESSAGE = "Internal service communication error, please report a bug"


async def schedule_ready_jobs():
    # GENERAL: strategy for robustness and failure recovery is, to always fetch all jobs with certain status
    # then think of all cases that can occur with a job in a certain state
    # this includes, that we have to think of cases that occur when unforseen errors happen
    try:
        jobs = await _fetch_jobs(["READY", "SCHEDULED", "RUNNING"])
    except CLIENT_EXCEPTIONS as e:
        _log_exception_info(e, "when retrieving jobs from MDS.")
        _http_exception_log(e)
    else:
        await _send_ready_jobs_to_jes(jobs)
        await _update_state_for_jobs_not_in_ready_state(jobs)


async def _fetch_jobs(statuses):
    jobs_object = await http_client.put(f"{mds_url}/v1/jobs/query", json={"statuses": statuses})
    return jobs_object["items"]


async def _send_ready_jobs_to_jes(jobs):
    for job in _jobs_ready(jobs):
        job_id = job["id"]
        job_status = "SCHEDULED"
        error_message = None

        try:
            job_token = await _fetch_raw_token(job)
            await _create_execution_request_raw(job=job, access_token=job_token["access_token"])
        except CLIENT_EXCEPTIONS as e:
            _log_exception_info(e, f"when executing READY job {job_id}.")
            _http_exception_log(e)
            job_status = "ERROR"
            error_message = GENERIC_ERROR_MESSAGE

        try:
            await _update_job_status(job_id=job_id, status=job_status, error_message=error_message)
        except CLIENT_EXCEPTIONS as e:
            _log_exception_info(
                e, f"when updating state of READY job {job_id}.")
            _http_exception_log(e)


async def _update_state_for_jobs_not_in_ready_state(jobs):
    for job in _jobs_not_ready(jobs):
        job_id = job["id"]
        get_execution_error = False
        get_execution_error_status_code = None
        try:
            execution = await _get_execution(job_id)
        except CustomHTTPException as e:
            _log_exception_info(
                e, f"when retrieving execution for non READY job {job_id} from JES.")
            _http_exception_log(e)
            get_execution_error = True
            get_execution_error_status_code = e.status_code
        except CLIENT_EXCEPTIONS as e:
            _log_exception_info(
                e, f"when retrieving execution for non READY job {job_id} from JES.")
            get_execution_error = True

        if get_execution_error and not settings.enable_eats_mode:
            if get_execution_error_status_code == 404:
                # probably JES DB has been dropped
                try:
                    await _update_job_status(job_id=job_id, status="ERROR", error_message=GENERIC_ERROR_MESSAGE)
                except CLIENT_EXCEPTIONS as e:
                    _log_exception_info(
                        e, f"when updating state of non READY job {job_id}.")
                    _http_exception_log(e)
            continue

        if execution["status"] == job["status"]:  # can be true for SCHEDULED or RUNNING
            continue

        terminal_state_messages = {
            "TERMINATED": "App terminated before finalizing its computation",
            "STOPPED": "App computation was stopped on request",
            "TIMEOUT": "App computation was stopped due to timeout",
            "ERROR": "Internal execution error, please report a bug",
        }

        if execution["status"] in terminal_state_messages:
            job_status = "ERROR"
            job_error_message = terminal_state_messages[execution["status"]]
        else:
            job_status = execution["status"]
            job_error_message = None

        try:
            # since terminal job statuses can't be overridden we can unconditionally try to update it
            # if the app already called /finalized or /failure this call will just be rejected
            # otherwise the job did neither do an explict call to /finalize nor /failure and has to be
            # updated accordintly
            await _update_job_status(job_id, job_status, job_error_message)
            logger.debug(f"Update job {job_id} state to {job_status}")
        except CLIENT_EXCEPTIONS as e:
            _log_exception_info(
                e,
                f"when updating status for non READY job {job_id} (not necessarily an error for terminal statuses).",
            )
            _http_exception_log(e)


def _http_exception_log(e):
    if isinstance(e, CustomHTTPException):
        logger.debug(f"{e.status_code}: {e.detail}")


def _log_exception_info(e, additional_message):
    logger.info(f"{type(e).__name__} {additional_message}")


def _jobs_ready(jobs):
    return filter(lambda job: job["status"] == "READY", jobs)


def _jobs_not_ready(jobs):
    return filter(lambda job: job["status"] != "READY", jobs)


async def _update_job_status(job_id, status, error_message):
    data = {"status": status, "error_message": error_message}
    return await http_client.put(f"{mds_url}/v1/jobs/{job_id}/status", json=data)


async def _get_execution(job_id):
    return await http_client.get(
        f"{jes_url}/v1/executions/{job_id}",
        headers={"organization-id": organization_id},
    )


async def _fetch_raw_token(job):
    job_id = job["id"]
    url = f"{mds_url}/v1/jobs/{job_id}/token"
    return await http_client.get(url)


async def _create_execution_request_raw(job, access_token, timeout=-1):
    execution_request = {
        "app_id": job["app_id"],
        "job_id": job["id"],
        "access_token": access_token,
        "app_service_url": as_url,
        "timeout": timeout,
    }
    url = f"{jes_url}/v1/executions"
    return await http_client.post(url, json=execution_request, headers={"organization-id": organization_id})
