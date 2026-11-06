import { z } from 'zod';

export const LoginFormSchema = z.object({
  username: z.string().min(1, 'username'),
  password: z.string().min(1, 'password'),
});


export type LoginFormValues = z.infer<typeof LoginFormSchema>;
