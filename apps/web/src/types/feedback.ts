export type FeedbackStatus = 
  | 'new'
  | 'under_review'
  | 'promoted_to_proposal'
  | 'promoted_to_issue'
  | 'declined';

export type FeedbackCategory =
  | 'protocol_mechanics'
  | 'developer_experience'
  | 'governance'
  | 'sme_onboarding'
  | 'investor_tools'
  | 'security';

export interface FeedbackComment {
  id: string;
  author: string;
  content: string;
  createdAt: string;
}

export interface FeedbackIdea {
  id: string;
  title: string;
  description: string;
  category: FeedbackCategory;
  author: string;
  upvotes: number;
  upvotedBy: string[];
  status: FeedbackStatus;
  statusReason?: string;
  promotedUrl?: string; // Link to governance proposal or Github Wave issue
  createdAt: string;
  updatedAt: string;
  triageNotes?: string;
  comments: FeedbackComment[];
}

export interface FeedbackSubmissionPayload {
  title: string;
  description: string;
  category: FeedbackCategory;
  author: string;
}

export interface TriageDecisionPayload {
  ideaId: string;
  status: FeedbackStatus;
  statusReason?: string;
  promotedUrl?: string;
  triageNotes?: string;
  triagedBy: string;
}

export interface TriageCadenceInfo {
  nextReviewDate: string;
  reviewFrequencyDays: number;
  minUpvotesForAutoReview: number;
  responsibleCommittee: string;
  guidelinesUrl: string;
}
