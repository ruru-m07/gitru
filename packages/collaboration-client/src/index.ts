import {
  collaborationAccounts,
  collaborationAcquireDemand,
  collaborationCapabilities,
  collaborationChangesSince,
  collaborationCommandRecoveryAction,
  collaborationCommandRecoveryDetail,
  collaborationCommandRecoveryExport,
  collaborationCommandRecoveryList,
  collaborationCommandRecoveryReplace,
  collaborationCommentDraft,
  collaborationCommentDrafts,
  collaborationConfirmLocalLink,
  collaborationConnectBitbucketCloud,
  collaborationConnectGithub,
  collaborationConnectGithubCli,
  collaborationConnectGitlab,
  collaborationContextualCapabilities,
  collaborationCreatedComments,
  collaborationDemandActivity,
  collaborationDetail,
  collaborationDiagnostics,
  collaborationDisconnect,
  collaborationDiscoverGithubCli,
  collaborationDiscoverNotificationSubject,
  collaborationDraft,
  collaborationDrafts,
  collaborationExecutePullCheckout,
  collaborationExportDiagnostics,
  collaborationExportDraft,
  collaborationGuardedMergeSnapshot,
  collaborationHydrateDetail,
  collaborationHydratePullFile,
  collaborationInbox,
  collaborationIssueDraft,
  collaborationIssueDrafts,
  collaborationIssueDraftsV2,
  collaborationIssueDraftV2,
  collaborationIssueMetadataOptions,
  collaborationItem,
  collaborationItems,
  collaborationLoadLocalPullFile,
  collaborationLocalClones,
  collaborationLocalLinks,
  collaborationNotificationSubject,
  collaborationOpenLocalPullCommit,
  collaborationPlanPullCheckout,
  collaborationPreviewGuardedMerge,
  collaborationPreviewPullCreation,
  collaborationProviderInboxActions,
  collaborationPullCommits,
  collaborationPullDraft,
  collaborationPullDrafts,
  collaborationPullFileArtifact,
  collaborationPullFiles,
  collaborationQueueProviderInboxAction,
  collaborationRefresh,
  collaborationRefreshIssueMetadata,
  collaborationReleaseDemand,
  collaborationRemoveLocalLink,
  collaborationRemoveTransportBinding,
  collaborationRenewDemand,
  collaborationRepositories,
  collaborationResolveResource,
  collaborationReviewDraft,
  collaborationReviewDrafts,
  collaborationSaveCommentDraft,
  collaborationSaveDraft,
  collaborationSaveIssueDraft,
  collaborationSaveIssueDraftV2,
  collaborationSavePullDraft,
  collaborationSaveReviewDraft,
  collaborationSaveTransportBinding,
  collaborationSelectRepository,
  collaborationSendComment,
  collaborationSetLocalInboxState,
  collaborationSubmitGuardedMerge,
  collaborationSubmitIssue,
  collaborationSubmitIssueV2,
  collaborationSubmitPull,
  collaborationSubmitReview,
  collaborationSubmitTextEdit,
  collaborationSubmittedReviews,
  collaborationSubmitWorkflowState,
  collaborationTextEditSnapshot,
  collaborationValidateLocalNavigation,
  collaborationWorkflowStateSnapshot,
} from "@gitru/commands";
import type { QueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { CollaborationClient } from "./client";

export type {
  AccountSyncDiagnostics,
  AcquireDemandRequest,
  ActivityEvent,
  CanonicalResource,
  CapabilitySnapshot,
  CapabilityTarget,
  CheckKind,
  CheckStateV1,
  CheckV1,
  CollaborationPullCheckoutReceipt as PullCheckoutReceipt,
  CommandFieldResolution,
  CommandFieldReview,
  CommandFieldValue,
  CommandRecoveryActionRequest,
  CommandRecoveryContext,
  CommandRecoveryDetail,
  CommandRecoveryQuery,
  CommandRecoveryReceipt,
  CommandRecoveryReplaceRequest,
  CommandRecoverySnapshot,
  CommandRecoverySummary,
  CommentDraftPage,
  CommentDraftQuery,
  CommentDraftSnapshot,
  CommentDraftSummary,
  CommentSendContext,
  CommentSubmissionReceipt,
  CommentSubmissionStatus,
  ContextCapabilityAccess,
  ContextCapabilityRequest,
  ContextFacetCapability,
  ContextualCapabilitySnapshot,
  CoverageDiagnostics,
  CreatedCommentPage,
  CreatedCommentQuery,
  CreatedCommentReceipt,
  CreatedIssueIdentity,
  CreatedPullIdentity,
  DemandLeaseReceipt,
  DemandOwnerActivity,
  DemandRenewalReceipt,
  DemandTarget,
  DetailActor,
  DetailBranch,
  DetailEntry,
  DetailEvidence,
  DetailFieldValidation,
  DetailLabel,
  DetailMilestone,
  DetailQuery,
  DetailSnapshot,
  DetailValue,
  DiscoverNotificationSubjectRequest,
  DraftPage,
  DraftQuery,
  DraftSummary,
  ExecutePullCheckoutRequest,
  GithubCliAccount,
  GithubCliDiscovery,
  GitlabDiscussionNoteV1,
  GitlabReviewLineRangeV1,
  GitlabReviewLineV1,
  GitlabReviewPositionV1,
  GuardedMergeContext,
  GuardedMergePreview,
  GuardedMergeQuery,
  GuardedMergeReceipt,
  GuardedMergeRequest,
  GuardedMergeSnapshot,
  GuardedMergeStatus,
  HydrateDetailRequest,
  InboxEntry,
  InboxPage,
  InboxQuery,
  IssueDraftContext,
  IssueDraftKey,
  IssueDraftPage,
  IssueDraftQuery,
  IssueDraftSnapshot,
  IssueDraftSummary,
  IssueDraftV2Page,
  IssueDraftV2Snapshot,
  IssueDraftV2Summary,
  IssueMetadataAssignee,
  IssueMetadataFieldOutcome,
  IssueMetadataLabel,
  IssueMetadataMilestone,
  IssueMetadataOption,
  IssueMetadataOutcome,
  IssueMetadataPage,
  IssueMetadataQuery,
  IssueMetadataReference,
  IssueMetadataSelection,
  IssueSubmissionReceipt,
  IssueSubmissionStatus,
  ItemQuery,
  LoadLocalPullFileRequest,
  LocalCloneRecord,
  LocalCloneSnapshot,
  LocalDraft,
  LocalInboxState,
  LocalInboxWriteReceipt,
  LocalLinkCandidate,
  LocalLinkInspection,
  LocalLinkVersion,
  LocalNavigationReceipt,
  LocalNavigationRequest,
  LocalRepositoryLink,
  LocalTransportBinding,
  MetadataFieldEvidence,
  NativeDetailPayload,
  NativeInboxState,
  NotificationSubjectQuery,
  NotificationSubjectSnapshot,
  OpenLocalPullCommitReceipt,
  OpenLocalPullCommitRequest,
  ParticipantUser,
  ParticipantV1,
  PreviewPullCreationRequest,
  ProviderInboxActionDescriptor,
  ProviderInboxActionReceipt,
  ProviderInboxActionsSnapshot,
  PullCheckoutPlan,
  PullCheckoutPlanRequest,
  PullCommit,
  PullCommitActor,
  PullCommitCompleteness,
  PullCommitContext,
  PullCommitMessage,
  PullCommitQuery,
  PullCommitSnapshot,
  PullCreationContext,
  PullCreationPreview,
  PullDraftKey,
  PullDraftPage,
  PullDraftQuery,
  PullDraftSnapshot,
  PullDraftSummary,
  PullDraftValues,
  PullFile,
  PullFileArtifact,
  PullFileArtifactSnapshot,
  PullFileBlobReferences,
  PullFileCapEvidence,
  PullFileCompleteness,
  PullFileContext,
  PullFileDiffRequest,
  PullFileIdentity,
  PullFileMembershipReceipt,
  PullFileQuery,
  PullFileSnapshot,
  PullFileSource,
  PullSubmissionReceipt,
  PullSubmissionStatus,
  QueueProviderInboxActionRequest,
  RefreshIssueMetadataRequest,
  ReleaseDemandRequest,
  RemoteAccount,
  RemoteItem,
  RemoteRepository,
  RenewDemandRequest,
  ResourceLocator,
  ResourceMetadataSnapshot,
  ResourceMetadataValues,
  ResourceResolution,
  ReviewActor,
  ReviewAnchor,
  ReviewAnchorSubject,
  ReviewContext,
  ReviewDecision,
  ReviewDiffSide,
  ReviewThreadNativeV1,
  ReviewThreadV1,
  ReviewV1,
  SaveCommentDraftRequest,
  SaveIssueDraftRequest,
  SaveIssueDraftV2Request,
  SavePullDraftRequest,
  SendCommentRequest,
  SetLocalInboxStateRequest,
  StorageDiagnostics,
  SubmitIssueRequest,
  SubmitIssueV2Request,
  SubmitPullRequest,
  SyncDiagnosticsExportReceipt,
  SyncDiagnosticsSnapshot,
  SyncLatencyDiagnostics,
  SyncRecoveryState,
  TaskActor,
  TaskV1,
  TextEditContext,
  TextEditReceipt,
  TextEditRequest,
  TextEditSnapshot,
  TodoCompletion,
  TransportBindingRequest,
  WorkflowStateContext,
  WorkflowStateReceipt,
  WorkflowStateRequest,
  WorkflowStateSnapshot,
} from "@gitru/commands";
export { StaleAuthorizationError } from "./authorization-fence";
export * from "./client";
export type { DemandAccount, DemandHandle } from "./demand-coordinator";
export type LocalLinkState =
  import("@gitru/commands").LocalRepositoryLink["state"];
export type RemoteItemKind = import("@gitru/commands").ItemQuery["kind"];
export type LocalInboxDisposition =
  import("@gitru/commands").LocalInboxState["disposition"];
export type LocalInboxEffectiveDisposition =
  import("@gitru/commands").LocalInboxState["effective_disposition"];
export type LocalInboxFilter =
  import("@gitru/commands").InboxQuery["local_state"];
export type LocalInboxMutation =
  import("@gitru/commands").SetLocalInboxStateRequest["mutation"];
export type DetailFacet = import("@gitru/commands").DetailQuery["facet"];
export type SyncRecoveryCategory =
  import("@gitru/commands").SyncRecoveryState["category"];
export type DetailField =
  import("@gitru/commands").DetailEntry["field_mask"][number];
export type CapabilityObservation =
  import("@gitru/commands").ContextFacetCapability["observation"];
export type ContextCapabilityReason = NonNullable<
  import("@gitru/commands").ContextCapabilityAccess["reason"]
>;
export type ProviderInboxAction =
  import("@gitru/commands").QueueProviderInboxActionRequest["action"];
export type ProviderInboxActionReason = NonNullable<
  import("@gitru/commands").ProviderInboxActionDescriptor["reason"]
>;
export type InboxSemantics =
  import("@gitru/commands").CapabilitySnapshot["inbox_semantics"];
export type ResourceFacet =
  import("@gitru/commands").ContextFacetCapability["facet"];
export type ResourceKind = import("@gitru/commands").ResourceLocator["kind"];
export type TextEditAvailability =
  import("@gitru/commands").TextEditSnapshot["availability"];
export type TextEditReason = NonNullable<
  import("@gitru/commands").TextEditSnapshot["reason"]
>;
export type WorkflowState =
  import("@gitru/commands").WorkflowStateRequest["desired_state"];
export type WorkflowStateAvailability =
  import("@gitru/commands").WorkflowStateSnapshot["availability"];
export type WorkflowStateReason = NonNullable<
  import("@gitru/commands").WorkflowStateSnapshot["reason"]
>;
export type CommentSendAvailability =
  import("@gitru/commands").CommentDraftSnapshot["availability"];
export type CommentSendReason = NonNullable<
  import("@gitru/commands").CommentDraftSnapshot["reason"]
>;
export type IssueDraftAvailability =
  import("@gitru/commands").IssueDraftSnapshot["availability"];
export type IssueDraftReason = NonNullable<
  import("@gitru/commands").IssueDraftSnapshot["reason"]
>;

export type PullCreationReason = NonNullable<
  import("@gitru/commands").PullDraftSnapshot["reason"]
>;

export const collaboration = new CollaborationClient({
  providerInboxActions: (query) => collaborationProviderInboxActions({ query }),
  queueProviderInboxAction: (request) =>
    collaborationQueueProviderInboxAction({ request }),
  commandRecoveryList: (query) => collaborationCommandRecoveryList({ query }),
  commandRecoveryDetail: (accountId, commandId) =>
    collaborationCommandRecoveryDetail({ accountId, commandId }),
  commandRecoveryAction: (request) =>
    collaborationCommandRecoveryAction({ request }),
  commandRecoveryReplace: (request) =>
    collaborationCommandRecoveryReplace({ request }),
  commandRecoveryExport: (context) =>
    collaborationCommandRecoveryExport({ context }),
  textEditSnapshot: (accountId, subjectId) =>
    collaborationTextEditSnapshot({ accountId, subjectId }),
  submitTextEdit: (request) => collaborationSubmitTextEdit({ request }),
  guardedMergeSnapshot: (query) => collaborationGuardedMergeSnapshot({ query }),
  previewGuardedMerge: (query) => collaborationPreviewGuardedMerge({ query }),
  submitGuardedMerge: (request) => collaborationSubmitGuardedMerge({ request }),
  workflowStateSnapshot: (accountId, subjectId) =>
    collaborationWorkflowStateSnapshot({ accountId, subjectId }),
  submitWorkflowState: (request) =>
    collaborationSubmitWorkflowState({ request }),
  commentDraft: (accountId, subjectId) =>
    collaborationCommentDraft({ accountId, subjectId }),
  commentDrafts: (query) => collaborationCommentDrafts({ query }),
  saveCommentDraft: (request) => collaborationSaveCommentDraft({ request }),
  sendComment: (request) => collaborationSendComment({ request }),
  createdComments: (query) => collaborationCreatedComments({ query }),
  reviewDraft: (key) => collaborationReviewDraft({ key }),
  reviewDrafts: (query) => collaborationReviewDrafts({ query }),
  saveReviewDraft: (request) => collaborationSaveReviewDraft({ request }),
  submitReview: (request) => collaborationSubmitReview({ request }),
  submittedReviews: (query) => collaborationSubmittedReviews({ query }),
  pullDraft: (key) => collaborationPullDraft({ key }),
  pullDrafts: (query) => collaborationPullDrafts({ query }),
  savePullDraft: (request) => collaborationSavePullDraft({ request }),
  previewPullCreation: (request) =>
    collaborationPreviewPullCreation({ request }),
  submitPull: (request) => collaborationSubmitPull({ request }),
  issueDraftV2: (key) => collaborationIssueDraftV2({ key }),
  issueDraftsV2: (query) => collaborationIssueDraftsV2({ query }),
  saveIssueDraftV2: (request) => collaborationSaveIssueDraftV2({ request }),
  submitIssueV2: (request) => collaborationSubmitIssueV2({ request }),
  issueMetadataOptions: (query) => collaborationIssueMetadataOptions({ query }),
  refreshIssueMetadata: (request) =>
    collaborationRefreshIssueMetadata({ request }),
  issueDraft: (key) => collaborationIssueDraft({ key }),
  issueDrafts: (query) => collaborationIssueDrafts({ query }),
  saveIssueDraft: (request) => collaborationSaveIssueDraft({ request }),
  submitIssue: (request) => collaborationSubmitIssue({ request }),
  demandActivity: () => collaborationDemandActivity({}),
  acquireDemand: (request) => collaborationAcquireDemand({ request }),
  renewDemand: (request) => collaborationRenewDemand({ request }),
  releaseDemand: (request) => collaborationReleaseDemand({ request }),
  listenDemandActivity: (onActivity) => {
    const ownerLabel = getCurrentWebview().label;
    return listen<{
      owner_label: string;
      activity: import("@gitru/commands").DemandOwnerActivity;
    }>("collaboration:owner-activity", ({ payload }) => {
      if (payload.owner_label === ownerLabel) onActivity(payload.activity);
    });
  },
  localLinks: (localRepositoryId) =>
    collaborationLocalLinks({ localRepositoryId }),
  confirmLocalLink: (request) => collaborationConfirmLocalLink({ request }),
  removeLocalLink: ({ id, generation }) =>
    collaborationRemoveLocalLink({ id, generation }),
  saveTransportBinding: (request) =>
    collaborationSaveTransportBinding({ request }),
  removeTransportBinding: ({ id, generation }, expectedBindingsGeneration) =>
    collaborationRemoveTransportBinding({
      id,
      generation,
      expectedBindingsGeneration,
    }),
  localClones: (request) => collaborationLocalClones({ request }),
  validateLocalNavigation: (request) =>
    collaborationValidateLocalNavigation({ request }),
  listenLocalChanges: (onWake) => listen("gitru://repository-changed", onWake),
  listenRuntimeReset: (onReset) =>
    listen("gitru:collaboration-runtime-reset", onReset),
  accounts: () => collaborationAccounts({}),
  diagnostics: () => collaborationDiagnostics({}),
  exportDiagnostics: () => collaborationExportDiagnostics({}),
  connectGithub: (token) => collaborationConnectGithub({ token }),
  connectGitlab: (token) => collaborationConnectGitlab({ token }),
  connectBitbucketCloud: (token) =>
    collaborationConnectBitbucketCloud({ token }),
  discoverGithubCli: () => collaborationDiscoverGithubCli({}),
  connectGithubCli: (candidateId) =>
    collaborationConnectGithubCli({ candidateId }),
  disconnect: (accountId) => collaborationDisconnect({ accountId }),
  repositories: (accountId) => collaborationRepositories({ accountId }),
  selectRepository: (accountId, repositoryId, selected) =>
    collaborationSelectRepository({ accountId, repositoryId, selected }),
  items: (query) => collaborationItems({ query }),
  inbox: (query) => collaborationInbox({ query }),
  setLocalInboxState: (request) => collaborationSetLocalInboxState({ request }),
  item: (accountId, itemId) => collaborationItem({ accountId, itemId }),
  refresh: (request) => collaborationRefresh({ request }),
  changesSince: (afterRevision) => collaborationChangesSince({ afterRevision }),
  saveDraft: (draft) => collaborationSaveDraft({ draft }),
  draft: (accountId, subjectId) => collaborationDraft({ accountId, subjectId }),
  drafts: (query) => collaborationDrafts({ query }),
  exportDraft: (accountId, subjectId, generation) =>
    collaborationExportDraft({ accountId, subjectId, generation }),
  capabilities: (accountId) => collaborationCapabilities({ accountId }),
  contextualCapabilities: (request) =>
    collaborationContextualCapabilities({ request }),
  resolveResource: (accountId, locator) =>
    collaborationResolveResource({ accountId, locator }),
  detail: (query) => collaborationDetail({ query }),
  pullCommits: (query) => collaborationPullCommits({ query }),
  pullFiles: (query) => collaborationPullFiles({ query }),
  pullFileArtifact: (request) => collaborationPullFileArtifact({ request }),
  hydratePullFile: (request) => collaborationHydratePullFile({ request }),
  loadLocalPullFile: (request) => collaborationLoadLocalPullFile({ request }),
  hydrateDetail: (request) => collaborationHydrateDetail({ request }),
  notificationSubject: (query) => collaborationNotificationSubject({ query }),
  discoverNotificationSubject: (request) =>
    collaborationDiscoverNotificationSubject({ request }),
  planPullCheckout: (request) => collaborationPlanPullCheckout({ request }),
  executePullCheckout: (request) =>
    collaborationExecutePullCheckout({ request }),
  openLocalPullCommit: (request) =>
    collaborationOpenLocalPullCommit({ request }),
  listen: (onWake) =>
    getCurrentWebview().listen<{ revision: string }>(
      "gitru:collaboration-change",
      onWake,
    ),
});

export function installCollaborationBridge(
  queryClient: QueryClient,
): () => void {
  const stop = collaboration.installBridge(queryClient);
  const wake = () => {
    void collaboration.wake();
  };
  const visible = () => {
    if (document.visibilityState === "visible") wake();
  };
  window.addEventListener("focus", wake);
  window.addEventListener("online", wake);
  document.addEventListener("visibilitychange", visible);
  return () => {
    window.removeEventListener("focus", wake);
    window.removeEventListener("online", wake);
    document.removeEventListener("visibilitychange", visible);
    stop();
  };
}

export type {
  ReviewDraftAnchor,
  ReviewDraftAnchorSelection,
  ReviewDraftComment,
  ReviewDraftCommentInput,
  ReviewDraftKey,
  ReviewDraftPage,
  ReviewDraftQuery,
  ReviewDraftSnapshot,
  ReviewDraftSummary,
  ReviewSubmissionContext,
  ReviewSubmissionEvent,
  ReviewSubmissionReason,
  ReviewSubmissionReceipt,
  ReviewSubmissionStatus,
  SaveReviewDraftRequest,
  SubmitReviewRequest,
  SubmittedReviewPage,
  SubmittedReviewQuery,
  SubmittedReviewReceipt,
} from "@gitru/commands";

export type IssueMetadataKind =
  import("@gitru/commands").IssueMetadataQuery["kind"];
