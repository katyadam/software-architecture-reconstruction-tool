package org.adamkattan.rest;

import io.smallrye.common.annotation.Blocking;
import io.smallrye.mutiny.Uni;
import io.smallrye.mutiny.infrastructure.Infrastructure;
import jakarta.inject.Inject;
import jakarta.transaction.Transactional;
import jakarta.validation.Valid;
import jakarta.ws.rs.*;
import jakarta.ws.rs.core.MediaType;
import jakarta.ws.rs.core.Response;
import org.adamkattan.model.callgraph.*;
import org.adamkattan.model.callgraph.algorithms.MethodReachability;
import org.adamkattan.service.CallGraphInputService;

import java.util.List;

@Path("/call-graph-inputs")
public class CallGraphController {

    @Inject
    CallGraphInputService callGraphInputService;

    @GET
    @Path("/project/{projectId}")
    @Produces(MediaType.APPLICATION_JSON)
    public Response getProjectAnalysisInputs(@PathParam("projectId") Long projectId) { // TODO: move to ProjectController
        List<CallGraphInput> projectCallGraphInputs = callGraphInputService.getProjectCallGraphInputs(projectId);

        var dtos = projectCallGraphInputs.stream()
                .map(CallGraphInput::toSimpleDto)
                .toList();
        return Response.ok(dtos).build();
    }

    @GET
    @Path("/{callGraphInputId}")
    @Produces(MediaType.APPLICATION_JSON)
    public Response getCallGraphInputById(@PathParam("callGraphInputId") Long callGraphInputId) {
        CallGraphInput callGraphInput = callGraphInputService.getCallGraphInputById(callGraphInputId);
        return Response.ok(CallGraphInput.toDto(callGraphInput)).build();
    }

    @GET
    @Path("/{callGraphInputId}/summary")
    @Produces(MediaType.APPLICATION_JSON)
    public Response getSummary(@PathParam("callGraphInputId") Long callGraphInputId) {
        CallGraphInputSummary summary = new CallGraphInputSummary(
                callGraphInputService.getCallGraphInputOutputsCount(callGraphInputId)
        );
        return Response.ok(summary)
                .build();
    }


    @PUT
    @Path("/{callGraphInputId}/method-reachability")
    @Produces(MediaType.APPLICATION_JSON)
    @Consumes(MediaType.APPLICATION_JSON)
    @Blocking
    public Uni<CallGraph> computeMethodReachability(
            @PathParam("callGraphInputId") Long callGraphInputId,
            @Valid CallGraphMethodKey callGraphMethodKey
    ) {
        CallGraphInput callGraphInput = callGraphInputService.getCallGraphInputById(callGraphInputId);
        return Uni.createFrom().item(() -> MethodReachability.compute(callGraphInput.callGraph, callGraphMethodKey))
                .runSubscriptionOn(Infrastructure.getDefaultExecutor());
    }

    @POST
    @Consumes(MediaType.APPLICATION_JSON)
    @Transactional
    public Response addCallGraphInput(@Valid CallGraphInputDto callGraphInputDto) {
        return Response.status(Response.Status.CREATED)
                .entity(callGraphInputService.addCallGraphInputToProject(callGraphInputDto))
                .build();
    }

    @DELETE
    @Path("/{id}")
    @Produces(MediaType.APPLICATION_JSON)
    @Transactional
    public Response deleteCallGraphInputById(@PathParam("id") Long id) {
        Long removedId = callGraphInputService.deleteCallGraphInputById(id);
        return Response.ok("Removed INPUT with id: " + removedId)
                .build();
    }

}