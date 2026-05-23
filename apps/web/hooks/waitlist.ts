import {
  type UseMutationOptions,
  type UseQueryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api } from "@/lib/api";

export type WaitlistValues = {
  username: string;
  email: string;
};

export type WaitlistResponse = {
  success: boolean;
  message: string;
  user?: {
    id: string;
    name: string;
    email: string;
  };
  joined?: number;
};

export const waitlistCountQueryKey = ["waitlist-count"] as const;

const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

type WaitlistCountOptions = Omit<
  UseQueryOptions<number, Error>,
  "queryKey" | "queryFn"
>;

type JoinWaitlistOptions = Omit<
  UseMutationOptions<WaitlistResponse, Error, WaitlistValues>,
  "mutationFn" | "onSuccess"
> & {
  onSuccess?: UseMutationOptions<
    WaitlistResponse,
    Error,
    WaitlistValues
  >["onSuccess"];
};

export function useGetWaitlistCount(options?: WaitlistCountOptions) {
  return useQuery<number, Error>({
    queryKey: waitlistCountQueryKey,
    queryFn: async () => {
      const { data, error } = await api.waitlist.count.get();
      if (error) {
        throw new Error("Failed to load waitlist count");
      }
      return data?.count ?? 0;
    },
    staleTime: 60_000,
    ...options,
  });
}

export function useJoinWaitList(options?: JoinWaitlistOptions) {
  const queryClient = useQueryClient();

  return useMutation<WaitlistResponse, Error, WaitlistValues>({
    mutationFn: async (values: WaitlistValues) => {
      const normalized = {
        username: values.username.trim(),
        email: values.email.trim(),
      };

      if (!normalized.username) {
        throw new Error("Name cannot be empty");
      }

      if (!normalized.email) {
        throw new Error("Email cannot be empty");
      }

      if (!emailRegex.test(normalized.email)) {
        throw new Error("Invalid email format");
      }

      const { data, error } = await api.waitlist.post({
        email: normalized.email,
        name: normalized.username,
      });

      if (error) {
        throw new Error("Failed to connect to server");
      }

      if (!data?.success) {
        throw new Error(data?.message || "Something went wrong");
      }

      return data;
    },
    ...options,
    onSuccess: (data, variables, context) => {
      void queryClient.invalidateQueries({
        queryKey: waitlistCountQueryKey,
      });
    },
  });
}
